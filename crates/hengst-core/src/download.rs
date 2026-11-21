use crate::paths::{remove_stale_temps, unique_temp, validate_sha1};
use crate::{Error, Result};
use futures::stream::{self, StreamExt, TryStreamExt};
use sha1::{Digest, Sha1};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const USER_AGENT: &str = concat!(
    "HengstClient/",
    env!("CARGO_PKG_VERSION"),
    " (github.com/clemensjl/hengst-client)"
);

/// Largest answer accepted for a `.sha1` sidecar.
pub const MAX_SIDECAR_BYTES: usize = 1024;
/// Largest answer accepted for JSON metadata (version lists, profiles, promotions, manifests).
pub const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
/// Largest file accepted from a download that has no size in its metadata.
pub const MAX_FILE_BYTES: usize = 512 * 1024 * 1024;
const MAX_REDIRECTS: usize = 10;

/// Decides whether a redirect from the last of `previous` to `next` may be followed: at most
/// `MAX_REDIRECTS` hops, and never from https to anything but https.
fn redirect_allowed(
    previous: &[reqwest::Url],
    next: &reqwest::Url,
) -> std::result::Result<(), String> {
    if previous.len() >= MAX_REDIRECTS {
        return Err(format!("too many redirects (more than {MAX_REDIRECTS})"));
    }
    if previous.last().is_some_and(|from| from.scheme() == "https") && next.scheme() != "https" {
        return Err(format!("refusing a redirect from https to {next}"));
    }
    Ok(())
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::custom(
            |attempt| match redirect_allowed(attempt.previous(), attempt.url()) {
                Ok(()) => attempt.follow(),
                Err(reason) => attempt.error(reason),
            },
        ))
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()
        .expect("static reqwest configuration is valid")
}

/// One file to fetch. `sha1 = None` means "existing file is good enough".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Download {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    /// Set by `fill_missing_sha1` when `sha1` came from the hash cache. Only such a hash may be
    /// replaced by a freshly fetched sidecar; one from metadata never is.
    pub sha1_from_cache: bool,
}

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    concurrency: usize,
    retries: u32,
    sha1_cache: Option<PathBuf>,
}

impl Downloader {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            concurrency: 16,
            retries: 3,
            sha1_cache: None,
        }
    }

    /// Remembers hashes fetched from `<url>.sha1` in this directory (see `Paths::sha1_cache`),
    /// keyed by the full artifact URL. Without it every `fill_missing_sha1` asks the server.
    pub fn with_sha1_cache(mut self, dir: PathBuf) -> Self {
        self.sha1_cache = Some(dir);
        self
    }

    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.concurrency = n.max(1);
        self
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Fetches all items (deduplicated by target path). Returns how many were actually downloaded.
    pub async fn fetch_all(&self, items: Vec<Download>) -> Result<usize> {
        let mut seen = HashSet::new();
        let unique: Vec<Download> = items
            .into_iter()
            .filter(|d| seen.insert(d.path.clone()))
            .collect();
        let results: Vec<bool> = stream::iter(unique)
            .map(|item| {
                let this = self.clone();
                async move { this.fetch_one(&item).await }
            })
            .buffer_unordered(self.concurrency)
            .try_collect()
            .await?;
        Ok(results.into_iter().filter(|d| *d).count())
    }

    /// Returns `Ok(false)` if the file already existed and was valid.
    pub async fn fetch_one(&self, item: &Download) -> Result<bool> {
        self.fetch_verified(item).await.map(|(fetched, _)| fetched)
    }

    /// Like `fetch_one`, and also returns the SHA-1 the file was verified against. When the
    /// item's hash came from the hash cache (`sha1_from_cache`) and the download does not match
    /// it, the cache entry is evicted, the hash fetched again from `<url>.sha1` and the download
    /// retried once with it (a republished artifact or an earlier bad answer). A hash from
    /// metadata is never replaced.
    pub async fn fetch_verified(&self, item: &Download) -> Result<(bool, Option<String>)> {
        match self.fetch_attempts(item).await {
            Err(e @ Error::Checksum { .. }) if item.sha1_from_cache => {
                if let Some(file) = self.cache_file(&item.url) {
                    let _ = tokio::fs::remove_file(file).await;
                }
                let (fresh, _) = sidecar_sha1(self, item).await?;
                if item.sha1.as_deref() == Some(fresh.as_str()) {
                    return Err(e);
                }
                let retry = Download {
                    sha1: Some(fresh.clone()),
                    sha1_from_cache: false,
                    ..item.clone()
                };
                Ok((self.fetch_attempts(&retry).await?, Some(fresh)))
            }
            other => other.map(|fetched| (fetched, item.sha1.clone())),
        }
    }

    fn cache_file(&self, url: &str) -> Option<PathBuf> {
        self.sha1_cache
            .as_ref()
            .map(|dir| dir.join(sha1_hex(url.as_bytes())))
    }

    async fn fetch_attempts(&self, item: &Download) -> Result<bool> {
        if is_valid(&item.path, item.sha1.as_deref()).await? {
            return Ok(false);
        }
        let mut attempt = 0;
        loop {
            match self.try_fetch(item).await {
                Ok(()) => return Ok(true),
                Err(e) if attempt + 1 < self.retries && is_retryable(&e) => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(300 * u64::from(attempt))).await;
                }
                // The local file is missing or wrong and the server cannot be reached.
                Err(Error::Http { source, .. }) => {
                    return Err(Error::Offline {
                        path: item.path.clone(),
                        url: item.url.clone(),
                        source,
                    })
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// `get_bytes_limited` with the downloader's retry policy for transient failures.
    async fn get_with_retries(&self, url: &str, limit: usize) -> Result<Vec<u8>> {
        let mut attempt = 0;
        loop {
            match get_bytes_limited(&self.client, url, limit).await {
                Err(e) if attempt + 1 < self.retries && is_retryable(&e) => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(300 * u64::from(attempt))).await;
                }
                other => return other,
            }
        }
    }

    async fn try_fetch(&self, item: &Download) -> Result<()> {
        let bytes = get_bytes_limited(&self.client, &item.url, MAX_FILE_BYTES).await?;
        if let Some(expected) = &item.sha1 {
            let actual = sha1_hex(&bytes);
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(Error::Checksum {
                    path: item.path.clone(),
                    expected: expected.clone(),
                    actual,
                });
            }
        }
        if let Some(parent) = item.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        remove_stale_temps(&item.path);
        let tmp = unique_temp(&item.path);
        tokio::fs::write(&tmp, &bytes).await?;
        if let Err(e) = tokio::fs::rename(&tmp, &item.path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e.into());
        }
        Ok(())
    }
}

/// Fills in `sha1` for items without one from `GET <url>.sha1`, remembered in the downloader's
/// hash cache under the SHA-1 of the full URL (never next to the library, so a hash learned for
/// one origin cannot vouch for a file from another). HTTP 404 is `Error::NoChecksum`; a body that
/// is not a SHA-1 is rejected, and so is a corrupt cache entry (it is fetched again).
pub async fn fill_missing_sha1(dl: &Downloader, items: &mut [Download]) -> Result<()> {
    let todo: Vec<(usize, Download)> = items
        .iter()
        .enumerate()
        .filter(|(_, d)| d.sha1.is_none())
        .map(|(i, d)| (i, d.clone()))
        .collect();
    let found: Vec<(usize, (String, bool))> = stream::iter(todo)
        .map(|(i, item)| async move { Ok::<_, Error>((i, sidecar_sha1(dl, &item).await?)) })
        .buffer_unordered(dl.concurrency)
        .try_collect()
        .await?;
    for (i, (sha1, from_cache)) in found {
        items[i].sha1 = Some(sha1);
        items[i].sha1_from_cache = from_cache;
    }
    Ok(())
}

/// The hash for `item` and whether it came from the cache.
async fn sidecar_sha1(dl: &Downloader, item: &Download) -> Result<(String, bool)> {
    // An old `<path>.sha1` from earlier versions is never read; drop it when we stand on it.
    let mut legacy = item.path.clone().into_os_string();
    legacy.push(".sha1");
    let _ = tokio::fs::remove_file(PathBuf::from(legacy)).await;

    let cache_file = dl.cache_file(&item.url);
    if let Some(file) = &cache_file {
        if let Ok(text) = tokio::fs::read_to_string(file).await {
            let hash = text.trim();
            if validate_sha1(hash).is_ok() {
                return Ok((hash.to_ascii_lowercase(), true));
            }
        }
    }
    let url = format!("{}.sha1", item.url);
    let body = match dl.get_with_retries(&url, MAX_SIDECAR_BYTES).await {
        Ok(b) => b,
        Err(Error::Status { status: 404, .. }) => {
            return Err(Error::NoChecksum {
                url: item.url.clone(),
            })
        }
        // Without a usable cached hash the file cannot be checked while offline.
        Err(Error::Http { url, source }) => {
            return Err(Error::Offline {
                path: item.path.clone(),
                url,
                source,
            })
        }
        Err(e) => return Err(e),
    };
    let text = String::from_utf8_lossy(&body);
    let hash = text.split_whitespace().next().unwrap_or("");
    if validate_sha1(hash).is_err() {
        return Err(Error::InvalidChecksumFile { url });
    }
    let hash = hash.to_ascii_lowercase();
    if let Some(file) = cache_file {
        // Best effort: the hash is verified in memory; a locked or unwritable cache only means
        // it is asked for again next time.
        let _ = write_cache_entry(&file, &hash).await;
    }
    Ok((hash, false))
}

async fn write_cache_entry(file: &Path, hash: &str) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = unique_temp(file);
    tokio::fs::write(&tmp, hash).await?;
    if let Err(e) = tokio::fs::rename(&tmp, file).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    Ok(())
}

/// Fetches JSON metadata (at most `MAX_JSON_BYTES`).
pub async fn get_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    get_bytes_limited(client, url, MAX_JSON_BYTES).await
}

/// Fetches a body of at most `limit` bytes. The limit is enforced on the bytes actually read,
/// so a missing or false Content-Length does not get around it.
pub async fn get_bytes_limited(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
) -> Result<Vec<u8>> {
    let http = |source| Error::Http {
        url: url.to_string(),
        source,
    };
    let too_large = || Error::TooLarge {
        url: url.to_string(),
        limit,
    };
    let mut resp = client.get(url).send().await.map_err(http)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(Error::Status {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }
    if resp.content_length().is_some_and(|n| n > limit as u64) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(http)? {
        if body.len() + chunk.len() > limit {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn is_retryable(e: &Error) -> bool {
    match e {
        Error::Http { .. } | Error::Checksum { .. } => true,
        Error::Status { status, .. } => *status >= 500 || *status == 429,
        _ => false,
    }
}

async fn is_valid(path: &Path, sha1: Option<&str>) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let Some(expected) = sha1 else {
        return Ok(true);
    };
    let p = path.to_owned();
    let actual = tokio::task::spawn_blocking(move || sha1_file(&p))
        .await
        .expect("hash task panicked")?;
    Ok(actual.eq_ignore_ascii_case(expected))
}

pub fn sha1_hex(data: &[u8]) -> String {
    hex::encode(Sha1::digest(data))
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn item(server: &MockServer, dir: &Path, name: &str, sha1: Option<String>) -> Download {
        Download {
            url: format!("{}/{name}", server.uri()),
            path: dir.join("sub").join(name),
            sha1,
            sha1_from_cache: false,
        }
    }

    fn url(s: &str) -> reqwest::Url {
        reqwest::Url::parse(s).unwrap()
    }

    #[test]
    fn redirect_policy_refuses_https_downgrade_and_long_chains() {
        let https = [url("https://a.example/x")];
        assert!(redirect_allowed(&https, &url("https://b.example/y")).is_ok());
        let err = redirect_allowed(&https, &url("http://b.example/y")).unwrap_err();
        assert!(err.contains("https"), "{err}");
        // http to http and http to https stay allowed.
        let http = [url("http://a.example/x")];
        assert!(redirect_allowed(&http, &url("http://b.example/y")).is_ok());
        assert!(redirect_allowed(&http, &url("https://b.example/y")).is_ok());
        let long: Vec<_> = (0..MAX_REDIRECTS)
            .map(|_| url("http://a.example/"))
            .collect();
        assert!(redirect_allowed(&long, &url("http://b.example/")).is_err());
    }

    #[tokio::test]
    async fn client_follows_http_redirects() {
        let server = MockServer::start().await;
        Mock::given(path("/old"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("location", format!("{}/new", server.uri())),
            )
            .mount(&server)
            .await;
        Mock::given(path("/new"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;
        let body = get_bytes(&http_client(), &format!("{}/old", server.uri()))
            .await
            .unwrap();
        assert_eq!(body, b"ok");
    }

    #[tokio::test]
    async fn reads_are_capped_on_bytes_read() {
        let server = MockServer::start().await;
        Mock::given(path("/big"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 200]))
            .mount(&server)
            .await;
        let u = format!("{}/big", server.uri());
        let ok = get_bytes_limited(&http_client(), &u, 200).await.unwrap();
        assert_eq!(ok.len(), 200);
        let err = get_bytes_limited(&http_client(), &u, 199)
            .await
            .unwrap_err();
        let m = err.to_string();
        assert!(matches!(&err, Error::TooLarge { limit: 199, .. }), "{m}");
        assert!(m.contains(&u) && m.contains("199"), "{m}");
    }

    #[tokio::test]
    async fn oversized_sidecar_is_refused() {
        let server = MockServer::start().await;
        Mock::given(path("/lib.jar.sha1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_bytes(vec![b'a'; MAX_SIDECAR_BYTES + 1]),
            )
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let mut items = vec![item(&server, dir.path(), "lib.jar", None)];
        let err = fill_missing_sha1(&Downloader::new(http_client()), &mut items)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, Error::TooLarge { limit, .. } if *limit == MAX_SIDECAR_BYTES),
            "{err}"
        );
    }

    #[tokio::test]
    async fn sidecar_request_is_retried_after_a_transient_error() {
        let server = MockServer::start().await;
        Mock::given(path("/lib.jar.sha1"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(path("/lib.jar.sha1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(HELLO_SHA1))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let mut items = vec![item(&server, dir.path(), "lib.jar", None)];
        fill_missing_sha1(&Downloader::new(http_client()), &mut items)
            .await
            .unwrap();
        assert_eq!(items[0].sha1.as_deref(), Some(HELLO_SHA1));
    }

    #[test]
    fn sha1_of_hello() {
        assert_eq!(
            sha1_hex(b"hello"),
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
        );
    }

    #[tokio::test]
    async fn downloads_verifies_and_skips_second_time() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/a.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let d = item(&server, dir.path(), "a.bin", Some(sha1_hex(b"hello")));
        let dl = Downloader::new(http_client());
        assert!(dl.fetch_one(&d).await.unwrap());
        assert_eq!(std::fs::read(&d.path).unwrap(), b"hello");
        assert!(!dl.fetch_one(&d).await.unwrap());
    }

    #[tokio::test]
    async fn rejects_wrong_checksum_and_leaves_no_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"evil".to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let d = item(&server, dir.path(), "b.bin", Some(sha1_hex(b"hello")));
        let err = Downloader::new(http_client())
            .fetch_one(&d)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
        assert!(!d.path.exists());
    }

    #[tokio::test]
    async fn retries_server_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let d = item(&server, dir.path(), "c.bin", None);
        assert!(Downloader::new(http_client()).fetch_one(&d).await.unwrap());
    }

    #[tokio::test]
    async fn does_not_retry_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let d = item(&server, dir.path(), "d.bin", None);
        let err = Downloader::new(http_client())
            .fetch_one(&d)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Status { status: 404, .. }));
    }

    const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";

    #[tokio::test]
    async fn fills_sha1_from_sidecar_and_caches_it() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/lib.jar.sha1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(format!("{HELLO_SHA1}  lib.jar\n")),
            )
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(dir.path().join("cache"));
        let mut items = vec![item(&server, dir.path(), "lib.jar", None)];
        fill_missing_sha1(&dl, &mut items).await.unwrap();
        assert_eq!(items[0].sha1.as_deref(), Some(HELLO_SHA1));
        let mut again = vec![item(&server, dir.path(), "lib.jar", None)];
        fill_missing_sha1(&dl, &mut again).await.unwrap();
        assert_eq!(again[0].sha1, items[0].sha1);
    }

    #[tokio::test]
    async fn missing_sidecar_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let dl = Downloader::new(http_client());
        let err = fill_missing_sha1(&dl, &mut [item(&server, dir.path(), "x.jar", None)])
            .await
            .unwrap_err();
        assert!(matches!(err, Error::NoChecksum { url } if url.ends_with("/x.jar")));
    }

    #[tokio::test]
    async fn rejects_garbage_sidecar() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>302 Found</html>"))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(dir.path().join("cache"));
        let mut items = vec![item(&server, dir.path(), "g.jar", None)];
        assert!(matches!(
            fill_missing_sha1(&dl, &mut items).await,
            Err(Error::InvalidChecksumFile { .. })
        ));
        assert!(items[0].sha1.is_none());
        assert!(!dir.path().join("sub").join("g.jar.sha1").exists());
        assert!(!dir.path().join("cache").exists());
    }

    async fn hello_server(route: &str, sha1_body: &str) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(route.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{route}.sha1")))
            .respond_with(ResponseTemplate::new(200).set_body_string(sha1_body.to_string()))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn planted_sidecar_next_to_a_library_is_not_used() {
        let server = hello_server("/lib.jar", HELLO_SHA1).await;
        let dir = tempfile::tempdir().unwrap();
        let mut it = item(&server, dir.path(), "lib.jar", None);
        std::fs::create_dir_all(it.path.parent().unwrap()).unwrap();
        std::fs::write(&it.path, b"evil").unwrap();
        let planted = it.path.with_file_name("lib.jar.sha1");
        std::fs::write(&planted, sha1_hex(b"evil")).unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(dir.path().join("cache"));
        fill_missing_sha1(&dl, std::slice::from_mut(&mut it))
            .await
            .unwrap();
        assert_eq!(it.sha1.as_deref(), Some(HELLO_SHA1));
        assert!(!planted.exists());
        assert!(dl.fetch_one(&it).await.unwrap());
        assert_eq!(std::fs::read(&it.path).unwrap(), b"hello");
    }

    #[tokio::test]
    async fn hashes_are_cached_per_full_url_not_per_destination() {
        let a = hello_server("/a/lib.jar", HELLO_SHA1).await;
        let b = hello_server("/b/lib.jar", &sha1_hex(b"other")).await;
        let dir = tempfile::tempdir().unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(dir.path().join("cache"));
        let dest = dir.path().join("lib.jar");
        let mk = |server: &MockServer, p: &str| Download {
            url: format!("{}{p}", server.uri()),
            path: dest.clone(),
            sha1: None,
            sha1_from_cache: false,
        };
        let mut first = [mk(&a, "/a/lib.jar")];
        fill_missing_sha1(&dl, &mut first).await.unwrap();
        let mut second = [mk(&b, "/b/lib.jar")];
        fill_missing_sha1(&dl, &mut second).await.unwrap();
        assert_eq!(first[0].sha1.as_deref(), Some(HELLO_SHA1));
        assert_eq!(second[0].sha1.as_deref(), Some(sha1_hex(b"other").as_str()));
        assert!(!dir.path().join("lib.jar.sha1").exists());
    }

    #[tokio::test]
    async fn corrupt_cache_entry_is_fetched_again() {
        let server = hello_server("/lib.jar", HELLO_SHA1).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let mut it = item(&server, dir.path(), "lib.jar", None);
        std::fs::create_dir_all(&cache).unwrap();
        let entry = cache.join(sha1_hex(it.url.as_bytes()));
        std::fs::write(&entry, "not a hash").unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(cache);
        fill_missing_sha1(&dl, std::slice::from_mut(&mut it))
            .await
            .unwrap();
        assert_eq!(it.sha1.as_deref(), Some(HELLO_SHA1));
        assert_eq!(std::fs::read_to_string(&entry).unwrap(), HELLO_SHA1);
    }

    #[tokio::test]
    async fn unwritable_cache_does_not_fail_the_lookup() {
        let server = hello_server("/lib.jar", HELLO_SHA1).await;
        let dir = tempfile::tempdir().unwrap();
        // The cache "directory" is a file, so the entry cannot be written.
        let cache = dir.path().join("cache");
        std::fs::write(&cache, b"not a directory").unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(cache);
        let mut it = item(&server, dir.path(), "lib.jar", None);
        fill_missing_sha1(&dl, std::slice::from_mut(&mut it))
            .await
            .unwrap();
        assert_eq!(it.sha1.as_deref(), Some(HELLO_SHA1));
        assert!(dl.fetch_one(&it).await.unwrap());
    }

    #[tokio::test]
    async fn stale_cached_hash_is_evicted_and_the_download_retried_once() {
        let server = hello_server("/lib.jar", HELLO_SHA1).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let mut it = item(&server, dir.path(), "lib.jar", None);
        std::fs::create_dir_all(&cache).unwrap();
        let entry = cache.join(sha1_hex(it.url.as_bytes()));
        std::fs::write(&entry, sha1_hex(b"an older release")).unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(cache);
        fill_missing_sha1(&dl, std::slice::from_mut(&mut it))
            .await
            .unwrap();
        assert_eq!(it.sha1, Some(sha1_hex(b"an older release")));
        let (fetched, sha) = dl.fetch_verified(&it).await.unwrap();
        assert!(fetched);
        assert_eq!(sha.as_deref(), Some(HELLO_SHA1));
        assert_eq!(std::fs::read(&it.path).unwrap(), b"hello");
        assert_eq!(std::fs::read_to_string(&entry).unwrap(), HELLO_SHA1);
    }

    #[tokio::test]
    async fn refetched_hash_that_still_mismatches_is_an_error() {
        // The server's sidecar does not match the file it serves.
        let server = hello_server("/lib.jar", &sha1_hex(b"what the sidecar claims")).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let mut it = item(&server, dir.path(), "lib.jar", None);
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            cache.join(sha1_hex(it.url.as_bytes())),
            sha1_hex(b"an older release"),
        )
        .unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(cache);
        fill_missing_sha1(&dl, std::slice::from_mut(&mut it))
            .await
            .unwrap();
        let err = dl.fetch_one(&it).await.unwrap_err();
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
        assert!(!it.path.exists());
        let sidecar_requests = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.path().ends_with(".sha1"))
            .count();
        assert_eq!(sidecar_requests, 1);
    }

    #[tokio::test]
    async fn hash_from_the_caller_is_never_replaced_by_a_sidecar() {
        let server = hello_server("/lib.jar", &sha1_hex(b"evil")).await;
        let dir = tempfile::tempdir().unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(dir.path().join("cache"));
        let it = item(&server, dir.path(), "lib.jar", Some(sha1_hex(b"trusted")));
        assert!(matches!(
            dl.fetch_one(&it).await,
            Err(Error::Checksum { .. })
        ));
        assert!(!server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path().ends_with(".sha1")));
    }

    #[tokio::test]
    async fn metadata_hash_equal_to_a_cached_one_is_never_replaced() {
        let server = hello_server("/lib.jar", HELLO_SHA1).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let stale = sha1_hex(b"an older release");
        let mut from_cache = item(&server, dir.path(), "lib.jar", None);
        std::fs::create_dir_all(&cache).unwrap();
        let entry = cache.join(sha1_hex(from_cache.url.as_bytes()));
        std::fs::write(&entry, &stale).unwrap();
        let dl = Downloader::new(http_client()).with_sha1_cache(cache);
        fill_missing_sha1(&dl, std::slice::from_mut(&mut from_cache))
            .await
            .unwrap();
        assert!(from_cache.sha1_from_cache);
        // The same URL with a hash from metadata (here the same value) is not retried.
        let metadata = item(&server, dir.path(), "lib.jar", Some(stale.clone()));
        assert!(!metadata.sha1_from_cache);
        assert!(matches!(
            dl.fetch_one(&metadata).await,
            Err(Error::Checksum { .. })
        ));
        assert_eq!(std::fs::read_to_string(&entry).unwrap(), stale);
        assert!(!server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path().ends_with(".sha1")));
    }

    #[tokio::test]
    async fn stale_temp_files_next_to_the_destination_are_removed() {
        let server = hello_server("/lib.jar", HELLO_SHA1).await;
        let dir = tempfile::tempdir().unwrap();
        let it = item(&server, dir.path(), "lib.jar", Some(HELLO_SHA1.into()));
        let sub = it.path.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&sub).unwrap();
        let old = std::time::SystemTime::now() - Duration::from_secs(3600);
        let mk = |name: &str, aged: bool| {
            let p = sub.join(name);
            let f = std::fs::File::create(&p).unwrap();
            if aged {
                f.set_modified(old).unwrap();
            }
            p
        };
        let stale = mk("lib.jar.999999.3.part", true);
        let fresh = mk("lib.jar.999999.4.part", false);
        let other = mk("other.jar.999999.3.part", true);
        let odd = mk("lib.jar.x.3.part", true);
        assert!(Downloader::new(http_client()).fetch_one(&it).await.unwrap());
        assert!(!stale.exists());
        assert!(fresh.exists() && other.exists() && odd.exists());
    }

    /// A URL on a port that was just free and has no listener.
    fn unreachable_url(name: &str) -> String {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        format!("http://127.0.0.1:{port}/{name}")
    }

    #[tokio::test]
    async fn offline_without_a_usable_cached_hash_is_the_offline_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let dl = Downloader::new(http_client()).with_sha1_cache(cache.clone());
        let url = unreachable_url("lib.jar");
        let it = Download {
            url: url.clone(),
            path: dir.path().join("lib.jar"),
            sha1: None,
            sha1_from_cache: false,
        };
        std::fs::write(&it.path, b"hello").unwrap();
        // No cache entry, then a corrupt one.
        for entry in [None, Some("not a hash")] {
            if let Some(text) = entry {
                std::fs::create_dir_all(&cache).unwrap();
                std::fs::write(cache.join(sha1_hex(url.as_bytes())), text).unwrap();
            }
            let err = fill_missing_sha1(&dl, &mut [it.clone()]).await.unwrap_err();
            let m = err.to_string();
            assert!(
                matches!(&err, Error::Offline { path, .. } if *path == it.path),
                "{m}"
            );
            assert!(m.contains("lib.jar") && m.contains("offline"), "{m}");
            // The cause is kept for the message and as the error source.
            assert!(std::error::Error::source(&err).is_some(), "{m}");
        }
    }

    #[tokio::test]
    async fn offline_download_names_the_file_and_keeps_the_cause() {
        let dir = tempfile::tempdir().unwrap();
        let it = Download {
            url: unreachable_url("a.bin"),
            path: dir.path().join("a.bin"),
            sha1: Some(HELLO_SHA1.into()),
            sha1_from_cache: false,
        };
        let err = Downloader::new(http_client())
            .fetch_one(&it)
            .await
            .unwrap_err();
        let m = err.to_string();
        assert!(matches!(&err, Error::Offline { .. }), "{m}");
        let source = std::error::Error::source(&err)
            .expect("cause kept")
            .to_string();
        assert!(m.contains(&source), "{m}");
    }

    #[tokio::test]
    async fn items_with_sha1_are_left_alone() {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let dl = Downloader::new(http_client());
        let mut items = vec![item(
            &server,
            dir.path(),
            "k.jar",
            Some(HELLO_SHA1.to_string()),
        )];
        let before = items.clone();
        fill_missing_sha1(&dl, &mut items).await.unwrap();
        assert_eq!(items, before);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn fetch_all_dedupes_by_path_and_counts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x".to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let a = item(&server, dir.path(), "a", None);
        let b = item(&server, dir.path(), "b", None);
        let n = Downloader::new(http_client())
            .fetch_all(vec![a.clone(), b, a])
            .await
            .unwrap();
        assert_eq!(n, 2);
    }
}
