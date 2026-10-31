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
        .timeout(Duration::from_secs(300))
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
