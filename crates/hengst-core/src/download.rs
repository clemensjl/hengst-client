use crate::paths::validate_sha1;
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

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
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
}

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    concurrency: usize,
    retries: u32,
}

impl Downloader {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            concurrency: 16,
            retries: 3,
        }
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
                Err(e) => return Err(e),
            }
        }
    }

    async fn try_fetch(&self, item: &Download) -> Result<()> {
        let bytes = get_bytes(&self.client, &item.url).await?;
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
        let mut tmp = item.path.clone().into_os_string();
        tmp.push(".part");
        let tmp = PathBuf::from(tmp);
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &item.path).await?;
        Ok(())
    }
}

/// Fills in `sha1` for items without one, from `<path>.sha1` if present, otherwise from
/// `GET <url>.sha1` (then stored as `<path>.sha1`). HTTP 404 is `Error::NoChecksum`; a body that
/// is not a SHA-1 is rejected.
pub async fn fill_missing_sha1(dl: &Downloader, items: &mut [Download]) -> Result<()> {
    let todo: Vec<(usize, Download)> = items
        .iter()
        .enumerate()
        .filter(|(_, d)| d.sha1.is_none())
        .map(|(i, d)| (i, d.clone()))
        .collect();
    let found: Vec<(usize, String)> = stream::iter(todo)
        .map(|(i, item)| async move { Ok::<_, Error>((i, sidecar_sha1(dl, &item).await?)) })
        .buffer_unordered(dl.concurrency)
        .try_collect()
        .await?;
    for (i, sha1) in found {
        items[i].sha1 = Some(sha1);
    }
    Ok(())
}

async fn sidecar_sha1(dl: &Downloader, item: &Download) -> Result<String> {
    let mut file = item.path.clone().into_os_string();
    file.push(".sha1");
    let file = PathBuf::from(file);
    if let Ok(text) = tokio::fs::read_to_string(&file).await {
        if let Some(hash) = text
            .split_whitespace()
            .next()
            .filter(|h| validate_sha1(h).is_ok())
        {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    let url = format!("{}.sha1", item.url);
    let body = match get_bytes(&dl.client, &url).await {
        Ok(b) => b,
        Err(Error::Status { status: 404, .. }) => {
            return Err(Error::NoChecksum {
                url: item.url.clone(),
            })
        }
        Err(e) => return Err(e),
    };
    let text = String::from_utf8_lossy(&body);
    let hash = text.split_whitespace().next().unwrap_or("");
    validate_sha1(hash)?;
    let hash = hash.to_ascii_lowercase();
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&file, &hash).await?;
    Ok(hash)
}

pub async fn get_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let http = |source| Error::Http {
        url: url.to_string(),
        source,
    };
    let resp = client.get(url).send().await.map_err(http)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(Error::Status {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }
    Ok(resp.bytes().await.map_err(http)?.to_vec())
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
        }
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
        let dl = Downloader::new(http_client());
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
        let dl = Downloader::new(http_client());
        let mut items = vec![item(&server, dir.path(), "g.jar", None)];
        assert!(matches!(
            fill_missing_sha1(&dl, &mut items).await,
            Err(Error::UnsafePath(_))
        ));
        assert!(items[0].sha1.is_none());
        assert!(!dir.path().join("sub").join("g.jar.sha1").exists());
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
