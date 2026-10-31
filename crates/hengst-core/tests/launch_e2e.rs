#![cfg(windows)]

use hengst_core::args::Account;
use hengst_core::download::sha1_hex;
use hengst_core::launch::{prepare, Endpoints, LaunchOptions};
use hengst_core::paths::Paths;
use serde_json::json;
use std::io::Write;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn native_jar() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        zip.start_file("lwjgl.dll", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"dll").unwrap();
        zip.finish().unwrap();
    }
    buf.into_inner()
}

async fn mount(server: &MockServer, p: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn prepares_a_full_version_against_mock_mojang() {
    let s = MockServer::start().await;
    let u = s.uri();
    let jar = native_jar();
    let asset_hash = sha1_hex(b"snd");
    let index = serde_json::to_vec(&json!({"objects": {"x.ogg": {"hash": asset_hash, "size": 3}}}))
        .unwrap();
    let runtime_manifest = serde_json::to_vec(&json!({"files": {
        "bin/java.exe": {"type": "file", "downloads": {"raw": {"sha1": sha1_hex(b"exe"), "size": 3, "url": format!("{u}/java.exe")}}}
    }})).unwrap();
    let version = serde_json::to_vec(&json!({
        "id": "9.9", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]},
        "assetIndex": {"id": "9", "sha1": sha1_hex(&index), "size": index.len(), "totalSize": 3, "url": format!("{u}/index.json")},
        "downloads": {"client": {"sha1": sha1_hex(b"client"), "size": 6, "url": format!("{u}/client.jar")}},
        "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
        "libraries": [{"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
            "downloads": {"artifact": {"path": "lwjgl-natives.jar", "sha1": sha1_hex(&jar), "size": jar.len(), "url": format!("{u}/lwjgl-natives.jar")}}}]
    })).unwrap();
    let manifest = json!({"latest": {"release": "9.9", "snapshot": "9.9"}, "versions": [
        {"id": "9.9", "type": "release", "url": format!("{u}/9.9.json"), "sha1": sha1_hex(&version), "releaseTime": "r"}
    ]});
    let runtimes = json!({"windows-x64": {"java-runtime-delta": [{"manifest": {
        "sha1": sha1_hex(&runtime_manifest), "size": runtime_manifest.len(), "url": format!("{u}/runtime.json")}}]}});

    mount(&s, "/manifest.json", serde_json::to_vec(&manifest).unwrap()).await;
    mount(&s, "/9.9.json", version).await;
    mount(&s, "/client.jar", b"client".to_vec()).await;
    mount(&s, "/lwjgl-natives.jar", jar).await;
    mount(&s, "/index.json", index).await;
    mount(
        &s,
        &format!("/{}/{}", &asset_hash[..2], asset_hash),
        b"snd".to_vec(),
    )
    .await;
    mount(&s, "/all.json", serde_json::to_vec(&runtimes).unwrap()).await;
    mount(&s, "/runtime.json", runtime_manifest).await;
    mount(&s, "/java.exe", b"exe".to_vec()).await;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let opts = LaunchOptions {
        version: "latest".into(),
        instance: None,
        account: Account::offline("Dev"),
        java: None,
        memory_mb: 1024,
    };
    let ep = Endpoints {
        manifest: format!("{u}/manifest.json"),
        resources: u.clone(),
        runtime_index: format!("{u}/all.json"),
    };
    let p = prepare(&paths, &opts, &ep).await.unwrap();

    assert_eq!(p.version_id, "9.9");
    assert_eq!(
        p.java,
        paths
            .java_runtime("java-runtime-delta")
            .join("bin")
            .join("java.exe")
    );
    assert_eq!(p.game_dir, paths.instance("9.9"));
    assert!(p
        .game_dir
        .join(".natives")
        .join("9.9")
        .join("lwjgl.dll")
        .is_file());
    let cp = &p.args[p.args.iter().position(|a| a == "-cp").unwrap() + 1];
    assert!(cp.ends_with(&paths.client_jar("9.9").to_string_lossy().into_owned()));
    assert!(cp.contains("lwjgl-natives.jar"));
    assert!(paths.asset_object(&asset_hash).is_file());
    assert_eq!(p.args.last().unwrap(), "Dev");
}
