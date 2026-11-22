//! Mock of the Mojang services (version manifest, version files, libraries, assets, Java runtime
//! index) and of the Fabric meta service. Everything is served below the mock's base URL; point
//! the core at it with [`endpoints`].

use hengst_core::download::sha1_hex;
use hengst_core::launch::Endpoints;
use hengst_core::loaders::LoaderEndpoints;
use serde_json::{json, Value};
use std::io::Write;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The versions the manifest lists: id and manifest type.
const VERSIONS: [(&str, &str); 4] = [
    ("9.9", "release"),
    ("9.9-snap", "snapshot"),
    ("b9.9", "old_beta"),
    ("a9.9", "old_alpha"),
];

const MANIFEST: &str = "/mojang/manifest.json";

fn native_jar() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        zip.start_file("lwjgl.dll", zip::write::SimpleFileOptions::default())
            .expect("in-memory zip");
        zip.write_all(b"dll").expect("in-memory zip");
        zip.finish().expect("in-memory zip");
    }
    buf.into_inner()
}

async fn serve(server: &MockServer, p: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

fn fabric_entry(version: &str, stable: bool) -> Value {
    json!({
        "loader": {
            "separator": ".",
            "build": 1,
            "maven": format!("net.fabricmc:fabric-loader:{version}"),
            "version": version,
            "stable": stable,
        }
    })
}

/// Mounts the version manifest (`9.9` release, `9.9-snap` snapshot, `b9.9` old beta, `a9.9` old
/// alpha, every file with a valid SHA-1) and the Fabric version lists (`9.9`: `0.99.0` stable and
/// `0.100.0-beta.1` unstable; `b9.9`: empty; other games: 404).
pub async fn mount_mojang(server: &MockServer) {
    let base = server.uri();
    let jar = native_jar();
    let asset_hash = sha1_hex(b"snd");
    let index = serde_json::to_vec(&json!({"objects": {"x.ogg": {"hash": asset_hash, "size": 3}}}))
        .expect("json");
    let runtime_manifest = serde_json::to_vec(&json!({"files": {
        "bin/java.exe": {"type": "file", "downloads": {"raw": {
            "sha1": sha1_hex(b"exe"), "size": 3, "url": format!("{base}/mojang/files/java.exe")}}}
    }}))
    .expect("json");
    let runtimes = json!({"windows-x64": {"java-runtime-delta": [{"manifest": {
        "sha1": sha1_hex(&runtime_manifest), "size": runtime_manifest.len(),
        "url": format!("{base}/mojang/runtime/manifest.json")}}]}});

    let mut entries = Vec::new();
    for (i, (id, kind)) in VERSIONS.iter().enumerate() {
        let version = serde_json::to_vec(&json!({
            "id": id, "type": kind, "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
                "game": ["--username", "${auth_player_name}"],
                "jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]
            },
            "assetIndex": {"id": "9", "sha1": sha1_hex(&index), "size": index.len(), "totalSize": 3,
                "url": format!("{base}/mojang/files/index.json")},
            "downloads": {"client": {"sha1": sha1_hex(b"client"), "size": 6,
                "url": format!("{base}/mojang/files/client.jar")}},
            "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
            "libraries": [{"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
                "downloads": {"artifact": {"path": "lwjgl-natives.jar", "sha1": sha1_hex(&jar),
                    "size": jar.len(), "url": format!("{base}/mojang/files/lwjgl-natives.jar")}}}]
        }))
        .expect("json");
        entries.push(json!({
            "id": id, "type": kind,
            "url": format!("{base}/mojang/versions/{id}.json"),
            "sha1": sha1_hex(&version),
            "releaseTime": format!("2099-01-0{}T00:00:00+00:00", 4 - i),
        }));
        serve(server, &format!("/mojang/versions/{id}.json"), version).await;
    }
    let manifest =
        json!({"latest": {"release": "9.9", "snapshot": "9.9-snap"}, "versions": entries});
    serve(
        server,
        MANIFEST,
        serde_json::to_vec(&manifest).expect("json"),
    )
    .await;
    serve(server, "/mojang/files/client.jar", b"client".to_vec()).await;
    serve(server, "/mojang/files/lwjgl-natives.jar", jar).await;
    serve(server, "/mojang/files/index.json", index).await;
    serve(server, "/mojang/files/java.exe", b"exe".to_vec()).await;
    serve(
        server,
        &format!("/mojang/resources/{}/{}", &asset_hash[..2], asset_hash),
        b"snd".to_vec(),
    )
    .await;
    serve(
        server,
        "/mojang/runtime/all.json",
        serde_json::to_vec(&runtimes).expect("json"),
    )
    .await;
    serve(server, "/mojang/runtime/manifest.json", runtime_manifest).await;

    let list = json!([
        fabric_entry("0.100.0-beta.1", false),
        fabric_entry("0.99.0", true)
    ]);
    serve(
        server,
        "/fabric/versions/loader/9.9",
        serde_json::to_vec(&list).expect("json"),
    )
    .await;
    serve(server, "/fabric/versions/loader/b9.9", b"[]".to_vec()).await;
}

/// The core's endpoints for a mock at `base`. Quilt, Legacy Fabric, Forge and NeoForge point at
/// unmounted paths below `base` (404), never at a real host.
pub fn endpoints(base: &str) -> Endpoints {
    let base = base.trim_end_matches('/');
    Endpoints {
        manifest: format!("{base}{MANIFEST}"),
        resources: format!("{base}/mojang/resources"),
        runtime_index: format!("{base}/mojang/runtime/all.json"),
        loaders: LoaderEndpoints {
            fabric: format!("{base}/fabric"),
            quilt: format!("{base}/quilt"),
            legacy_fabric: format!("{base}/legacy-fabric"),
            forge_files: format!("{base}/forge-files"),
            forge_maven: format!("{base}/forge-maven"),
            neoforge_maven: format!("{base}/neoforge-maven"),
        },
    }
}
