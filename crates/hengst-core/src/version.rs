use crate::download::{Download, Downloader};
use crate::manifest::VersionEntry;
use crate::paths::Paths;
use crate::rules::Rule;
use crate::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub main_class: String,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub downloads: Option<VersionDownloads>,
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub logging: Option<Logging>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Clone, Debug)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VersionDownloads {
    pub client: Option<FileRef>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct FileRef {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub url: Option<String>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Artifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Loads the version JSON, using the verified local copy when its SHA1 still matches.
pub async fn fetch_version(
    dl: &Downloader,
    paths: &Paths,
    entry: &VersionEntry,
) -> Result<VersionJson> {
    let path = paths.version_json(&entry.id);
    dl.fetch_one(&Download {
        url: entry.url.clone(),
        path: path.clone(),
        sha1: Some(entry.sha1.clone()),
    })
    .await?;
    Ok(serde_json::from_slice(&tokio::fs::read(&path).await?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_modern_version() {
        let v: VersionJson = serde_json::from_value(json!({
            "id": "1.21.11",
            "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
                "game": ["--username", "${auth_player_name}",
                    {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
                     "value": ["--width", "${resolution_width}"]}],
                "jvm": [{"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": "-XX:HeapDumpPath=x"},
                    "-cp", "${classpath}"]
            },
            "assetIndex": {"id": "29", "sha1": "a", "size": 1, "totalSize": 2, "url": "http://x/29.json"},
            "assets": "29",
            "downloads": {"client": {"sha1": "c", "size": 3, "url": "http://x/client.jar"}},
            "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
            "libraries": [{"name": "com.mojang:brigadier:1.3.10",
                "downloads": {"artifact": {"path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar", "sha1": "b", "size": 4, "url": "http://x/b.jar"}}}],
            "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}",
                "file": {"id": "client-1.21.2.xml", "sha1": "l", "size": 5, "url": "http://x/log.xml"}, "type": "log4j2-xml"}}
        }))
        .unwrap();
        let args = v.arguments.as_ref().unwrap();
        assert_eq!(args.game.len(), 3);
        assert!(
            matches!(&args.game[2], Argument::Conditional { value: ArgValue::Many(v), .. } if v.len() == 2)
        );
        assert!(matches!(
            &args.jvm[0],
            Argument::Conditional {
                value: ArgValue::One(_),
                ..
            }
        ));
        assert_eq!(v.java_version.as_ref().unwrap().major_version, 21);
        assert_eq!(
            v.libraries[0]
                .downloads
                .as_ref()
                .unwrap()
                .artifact
                .as_ref()
                .unwrap()
                .sha1,
            "b"
        );
        assert_eq!(
            v.logging.unwrap().client.unwrap().file.id,
            "client-1.21.2.xml"
        );
    }

    #[test]
    fn parses_legacy_version() {
        let v: VersionJson = serde_json::from_value(json!({
            "id": "1.8.9",
            "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name} --version ${version_name}",
            "assetIndex": {"id": "1.8", "sha1": "a", "size": 1, "totalSize": 2, "url": "http://x/1.8.json"},
            "libraries": [{
                "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
                "natives": {"windows": "natives-windows", "linux": "natives-linux"},
                "extract": {"exclude": ["META-INF/"]},
                "downloads": {"classifiers": {"natives-windows": {"path": "p.jar", "sha1": "n", "size": 1, "url": "http://x/n.jar"}}}
            }]
        }))
        .unwrap();
        assert!(v.arguments.is_none());
        assert!(v.java_version.is_none());
        assert!(v.minecraft_arguments.unwrap().contains("${version_name}"));
        let lib = &v.libraries[0];
        assert_eq!(lib.natives.as_ref().unwrap()["windows"], "natives-windows");
        assert_eq!(
            lib.extract.as_ref().unwrap().exclude,
            vec!["META-INF/".to_string()]
        );
    }
}
