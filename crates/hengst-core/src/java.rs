use crate::download::{get_bytes, Download, Downloader};
use crate::paths::Paths;
use crate::rules::Env;
use crate::version::VersionJson;
use crate::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

pub const RUNTIME_INDEX_URL: &str =
    "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: ManifestRef,
}

#[derive(Deserialize)]
struct ManifestRef {
    sha1: String,
    url: String,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    downloads: Option<RuntimeDownloads>,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: RawFile,
}

#[derive(Deserialize)]
struct RawFile {
    sha1: String,
    url: String,
}

pub fn platform_key(env: &Env) -> &'static str {
    match (env.os_name.as_str(), env.arch.as_str()) {
        ("windows", "x86") => "windows-x86",
        ("windows", "arm64") => "windows-arm64",
        ("windows", _) => "windows-x64",
        ("osx", "arm64") => "mac-os-arm64",
        ("osx", _) => "mac-os",
        ("linux", "x86") => "linux-i386",
        _ => "linux",
    }
}

/// Versions without `javaVersion` (before 1.17) run on Java 8 (`jre-legacy`).
pub fn component_for(v: &VersionJson) -> String {
    v.java_version
        .as_ref()
        .map(|j| j.component.clone())
        .unwrap_or_else(|| "jre-legacy".to_string())
}

/// `java.exe` keeps game output on the console for the CLI. The desktop app will switch to `javaw.exe`.
pub fn java_executable(paths: &Paths, env: &Env, component: &str) -> PathBuf {
    let bin = paths.java_runtime(component).join("bin");
    if env.os_name == "windows" {
        bin.join("java.exe")
    } else {
        bin.join("java")
    }
}

pub async fn ensure_runtime(
    dl: &Downloader,
    paths: &Paths,
    env: &Env,
    component: &str,
    index_url: &str,
) -> Result<PathBuf> {
    let exe = java_executable(paths, env, component);
    let index: HashMap<String, HashMap<String, Vec<RuntimeEntry>>> =
        match get_bytes(dl.client(), index_url).await {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e @ (Error::Http { .. } | Error::Status { .. })) => {
                return if exe.is_file() { Ok(exe) } else { Err(e) };
            }
            Err(e) => return Err(e),
        };
    let platform = platform_key(env);
    let entry = index
        .get(platform)
        .and_then(|m| m.get(component))
        .and_then(|list| list.first())
        .ok_or_else(|| Error::NoRuntime {
            component: component.to_string(),
            platform: platform.to_string(),
        })?;

    let root = paths.java_runtime(component);
    let manifest_path = root.join(".manifest.json");
    dl.fetch_one(&Download {
        url: entry.manifest.url.clone(),
        path: manifest_path.clone(),
        sha1: Some(entry.manifest.sha1.clone()),
    })
    .await?;
    let manifest: RuntimeManifest =
        serde_json::from_slice(&tokio::fs::read(&manifest_path).await?)?;

    let mut downloads = Vec::new();
    for (rel, file) in &manifest.files {
        if rel.contains("..") {
            continue;
        }
        match (file.kind.as_str(), &file.downloads) {
            ("directory", _) => tokio::fs::create_dir_all(root.join(rel)).await?,
            ("file", Some(d)) => downloads.push(Download {
                url: d.raw.url.clone(),
                path: root.join(rel),
                sha1: Some(d.raw.sha1.clone()),
            }),
            // Links only matter on macOS/Linux, which are out of MVP scope.
            _ => {}
        }
    }
    dl.fetch_all(downloads).await?;
    Ok(exe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::{http_client, sha1_hex};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn server() -> MockServer {
        let s = MockServer::start().await;
        let manifest = serde_json::to_vec(&json!({"files": {
            "bin": {"type": "directory"},
            "bin/java.exe": {"type": "file", "executable": true, "downloads": {
                "raw": {"sha1": sha1_hex(b"exe"), "size": 3, "url": format!("{}/java.exe", s.uri())}}},
            "legal/link": {"type": "link", "target": "../x"}
        }})).unwrap();
        let index = json!({"windows-x64": {
            "java-runtime-delta": [{"manifest": {"sha1": sha1_hex(&manifest), "size": manifest.len(), "url": format!("{}/manifest.json", s.uri())},
                                    "version": {"name": "21.0.7", "released": "x"}}],
            "jre-legacy": []
        }});
        Mock::given(method("GET"))
            .and(path("/all.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(index))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(manifest))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/java.exe"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"exe".to_vec()))
            .mount(&s)
            .await;
        s
    }

    #[test]
    fn component_defaults_to_legacy() {
        let v: VersionJson =
            serde_json::from_value(json!({"id": "1.8.9", "mainClass": "M"})).unwrap();
        assert_eq!(component_for(&v), "jre-legacy");
        assert_eq!(platform_key(&Env::windows_x64()), "windows-x64");
    }

    #[tokio::test]
    async fn installs_runtime_and_returns_executable() {
        let s = server().await;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let env = Env::windows_x64();
        let exe = ensure_runtime(
            &Downloader::new(http_client()),
            &paths,
            &env,
            "java-runtime-delta",
            &format!("{}/all.json", s.uri()),
        )
        .await
        .unwrap();
        assert_eq!(
            exe,
            paths
                .java_runtime("java-runtime-delta")
                .join("bin")
                .join("java.exe")
        );
        assert_eq!(std::fs::read(&exe).unwrap(), b"exe");
    }

    #[tokio::test]
    async fn missing_component_is_an_error() {
        let s = server().await;
        let dir = tempfile::tempdir().unwrap();
        let err = ensure_runtime(
            &Downloader::new(http_client()),
            &Paths::new(dir.path()),
            &Env::windows_x64(),
            "jre-legacy",
            &format!("{}/all.json", s.uri()),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::NoRuntime { .. }));
    }

    #[tokio::test]
    async fn offline_uses_installed_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let env = Env::windows_x64();
        let exe = java_executable(&paths, &env, "jre-legacy");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"x").unwrap();
        let got = ensure_runtime(
            &Downloader::new(http_client()),
            &paths,
            &env,
            "jre-legacy",
            "http://127.0.0.1:9/all.json",
        )
        .await
        .unwrap();
        assert_eq!(got, exe);
    }
}
