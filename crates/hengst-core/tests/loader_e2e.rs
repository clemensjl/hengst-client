#![cfg(windows)]

use hengst_core::args::Account;
use hengst_core::download::sha1_hex;
use hengst_core::launch::{prepare, Endpoints, LaunchOptions};
use hengst_core::libraries::maven_path;
use hengst_core::loaders::{LoaderEndpoints, LoaderKind, LoaderRequest};
use hengst_core::paths::Paths;
use hengst_core::Error;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FABRIC_LIST: &str = include_str!("fixtures/loaders/fabric-loaders-1.21.11.json");
const FABRIC_PROFILE: &str = include_str!("fixtures/loaders/fabric-profile-1.21.11-0.17.3.json");
const LEGACY_LIST: &str = include_str!("fixtures/loaders/legacy-fabric-loaders-1.8.9.json");
const LEGACY_PROFILE: &str =
    include_str!("fixtures/loaders/legacy-fabric-profile-1.8.9-0.20.1.json");

fn zip_with(name: &str, body: &[u8]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(body).unwrap();
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

fn opts(version: &str, loader: Option<(LoaderKind, Option<&str>)>) -> LaunchOptions {
    LaunchOptions {
        version: version.into(),
        instance: None,
        account: Account::offline("Dev"),
        java: None,
        memory_mb: 1024,
        loader: loader.map(|(kind, v)| LoaderRequest {
            kind,
            version: v.map(String::from),
        }),
    }
}

fn endpoints(u: &str) -> Endpoints {
    Endpoints {
        manifest: format!("{u}/manifest.json"),
        resources: u.to_string(),
        runtime_index: format!("{u}/all.json"),
        loaders: LoaderEndpoints {
            fabric: format!("{u}/fabric/v2"),
            quilt: format!("{u}/quilt/v3"),
            legacy_fabric: format!("{u}/legacy/v2"),
            ..LoaderEndpoints::default()
        },
    }
}

async fn mount_manifest(s: &MockServer, id: &str, version: Vec<u8>) {
    let u = s.uri();
    let manifest = json!({"latest": {"release": id, "snapshot": id}, "versions": [
        {"id": id, "type": "release", "url": format!("{u}/{id}.json"), "sha1": sha1_hex(&version), "releaseTime": "r"}
    ]});
    mount(s, "/manifest.json", serde_json::to_vec(&manifest).unwrap()).await;
    mount(s, &format!("/{id}.json"), version).await;
}

/// Vanilla "9.9": modern arguments, one native library, assets and a managed Java runtime.
async fn mount_modern_vanilla(s: &MockServer) {
    let u = s.uri();
    let jar = zip_with("lwjgl.dll", b"dll");
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
    let runtimes = json!({"windows-x64": {"java-runtime-delta": [{"manifest": {
        "sha1": sha1_hex(&runtime_manifest), "size": runtime_manifest.len(), "url": format!("{u}/runtime.json")}}]}});

    mount_manifest(s, "9.9", version).await;
    mount(s, "/client.jar", b"client".to_vec()).await;
    mount(s, "/lwjgl-natives.jar", jar).await;
    mount(s, "/index.json", index).await;
    mount(
        s,
        &format!("/{}/{}", &asset_hash[..2], asset_hash),
        b"snd".to_vec(),
    )
    .await;
    mount(s, "/all.json", serde_json::to_vec(&runtimes).unwrap()).await;
    mount(s, "/runtime.json", runtime_manifest).await;
    mount(s, "/java.exe", b"exe".to_vec()).await;
}

/// Vanilla "8.8": `minecraftArguments`, LWJGL 2 with a Mojang-style natives classifier.
async fn mount_legacy_vanilla(s: &MockServer) {
    let u = s.uri();
    let natives = zip_with("lwjgl64.dll", b"vanilla-dll");
    let version = serde_json::to_vec(&json!({
        "id": "8.8", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "minecraftArguments": "--username ${auth_player_name}",
        "downloads": {"client": {"sha1": sha1_hex(b"client"), "size": 6, "url": format!("{u}/client.jar")}},
        "libraries": [
            {"name": "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209",
             "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/lwjgl/2.9.4-nightly-20150209/lwjgl-2.9.4-nightly-20150209.jar",
                "sha1": sha1_hex(b"vanilla-lwjgl"), "size": 13, "url": format!("{u}/vanilla/lwjgl.jar")}}},
            {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
             "natives": {"windows": "natives-windows"},
             "downloads": {"classifiers": {"natives-windows": {"path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209-natives-windows.jar",
                "sha1": sha1_hex(&natives), "size": natives.len(), "url": format!("{u}/vanilla/natives.jar")}}}}
        ]
    })).unwrap();
    mount_manifest(s, "8.8", version).await;
    mount(s, "/client.jar", b"client".to_vec()).await;
    mount(s, "/vanilla/lwjgl.jar", b"vanilla-lwjgl".to_vec()).await;
    mount(s, "/vanilla/natives.jar", natives).await;
}

struct Jar {
    rel: String,
    body: Vec<u8>,
    /// The profile carries the hash itself; otherwise only a `.sha1` sidecar can vouch for it.
    inline_hash: bool,
}

/// Rewrites a profile fixture so it points at the mock server: new id and parent, every library
/// served from `{u}/maven/` with real bytes. Libraries that had a hash in the fixture keep one
/// (recomputed), the others rely on a sidecar.
fn mock_profile(fixture: &str, id: &str, game: &str, u: &str) -> (Value, Vec<Jar>) {
    let mut profile: Value = serde_json::from_str(fixture).unwrap();
    profile["id"] = json!(id);
    profile["inheritsFrom"] = json!(game);
    let mut jars = Vec::new();
    for lib in profile["libraries"].as_array_mut().unwrap() {
        let name = lib["name"].as_str().unwrap().to_string();
        let inline_hash = lib.get("sha1").is_some();
        let (rel, body) = if lib.get("natives").is_some() {
            (
                maven_path(&format!("{name}:natives-windows")).unwrap(),
                zip_with("lwjgl64.dll", b"legacy-dll"),
            )
        } else {
            (maven_path(&name).unwrap(), name.clone().into_bytes())
        };
        let obj = lib.as_object_mut().unwrap();
        for k in ["md5", "sha1", "sha256", "sha512", "size"] {
            obj.remove(k);
        }
        obj.insert("url".into(), json!(format!("{u}/maven/")));
        if inline_hash {
            obj.insert("sha1".into(), json!(sha1_hex(&body)));
        }
        jars.push(Jar {
            rel,
            body,
            inline_hash,
        });
    }
    (profile, jars)
}

async fn mount_jars(s: &MockServer, jars: &[Jar], skip_sidecar: &[&str]) {
    for j in jars {
        mount(s, &format!("/maven/{}", j.rel), j.body.clone()).await;
        if !j.inline_hash && !skip_sidecar.iter().any(|k| j.rel.contains(k)) {
            mount(
                s,
                &format!("/maven/{}.sha1", j.rel),
                sha1_hex(&j.body).into_bytes(),
            )
            .await;
        }
    }
}

async fn mount_fabric(s: &MockServer, profile: &Value) {
    mount(
        s,
        "/fabric/v2/versions/loader/9.9",
        FABRIC_LIST.as_bytes().to_vec(),
    )
    .await;
    mount(
        s,
        "/fabric/v2/versions/loader/9.9/0.20.1/profile/json",
        serde_json::to_vec(profile).unwrap(),
    )
    .await;
}

fn classpath(args: &[String]) -> &String {
    &args[args.iter().position(|a| a == "-cp").unwrap() + 1]
}

fn requested(s_requests: &[wiremock::Request]) -> Vec<String> {
    s_requests
        .iter()
        .map(|r| r.url.path().to_string())
        .collect()
}

#[tokio::test]
async fn prepares_fabric_on_a_modern_version() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_modern_vanilla(&s).await;
    let (mut profile, jars) = mock_profile(FABRIC_PROFILE, "fabric-loader-0.20.1-9.9", "9.9", &u);
    // A runtime the mock does not serve: Java must still follow the vanilla parent.
    profile["javaVersion"] = json!({"component": "java-runtime-alpha", "majorVersion": 8});
    mount_jars(&s, &jars, &[]).await;
    mount_fabric(&s, &profile).await;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let p = prepare(
        &paths,
        &opts("9.9", Some((LoaderKind::Fabric, None))),
        &endpoints(&u),
    )
    .await
    .unwrap();

    assert_eq!(p.version_id, "fabric-loader-0.20.1-9.9");
    assert_eq!(p.game_dir, paths.instance("9.9-fabric"));
    assert_eq!(p.loader.as_ref().unwrap().version.id, "0.20.1");
    let main = p
        .args
        .iter()
        .position(|a| a == "net.fabricmc.loader.impl.launch.knot.KnotClient")
        .unwrap();
    let emu = p
        .args
        .iter()
        .position(|a| a == "-DFabricMcEmu= net.minecraft.client.main.Main ")
        .unwrap();
    assert!(emu < main);
    let cp = classpath(&p.args);
    assert!(cp.contains("fabric-loader-0.17.3.jar"));
    let loader_jar = paths.client_jar("fabric-loader-0.20.1-9.9");
    assert!(cp.ends_with(&loader_jar.to_string_lossy().into_owned()));
    assert_eq!(std::fs::read(&loader_jar).unwrap(), b"client");
    assert_eq!(std::fs::read(paths.client_jar("9.9")).unwrap(), b"client");
    assert!(cp.contains("lwjgl-natives.jar"));
    assert!(p
        .game_dir
        .join(".natives")
        .join("fabric-loader-0.20.1-9.9")
        .join("lwjgl.dll")
        .is_file());
    assert_eq!(
        p.java,
        paths
            .java_runtime("java-runtime-delta")
            .join("bin")
            .join("java.exe")
    );
}

#[tokio::test]
async fn prepares_legacy_fabric_on_a_legacy_version() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_legacy_vanilla(&s).await;
    let (profile, jars) = mock_profile(LEGACY_PROFILE, "fabric-loader-0.20.1-8.8", "8.8", &u);
    mount_jars(&s, &jars, &[]).await;
    mount(
        &s,
        "/legacy/v2/versions/loader/8.8",
        LEGACY_LIST.as_bytes().to_vec(),
    )
    .await;
    mount(
        &s,
        "/legacy/v2/versions/loader/8.8/0.20.1/profile/json",
        serde_json::to_vec(&profile).unwrap(),
    )
    .await;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut o = opts("8.8", Some((LoaderKind::LegacyFabric, Some("0.20.1"))));
    o.java = Some(PathBuf::from("java.exe"));
    let p = prepare(&paths, &o, &endpoints(&u)).await.unwrap();

    assert_eq!(p.game_dir, paths.instance("8.8-legacy-fabric"));
    let cp = classpath(&p.args);
    assert!(cp.contains("lwjgl-2.9.4+legacyfabric.17.jar"));
    assert!(!cp.contains("2.9.4-nightly-20150209"));
    let natives = p.game_dir.join(".natives").join(&p.version_id);
    assert_eq!(
        std::fs::read(natives.join("lwjgl64.dll")).unwrap(),
        b"legacy-dll"
    );
    assert!(p
        .args
        .iter()
        .any(|a| a.as_str() == format!("-Djava.library.path={}", natives.to_string_lossy())));
    // The vanilla LWJGL jars were replaced, not downloaded.
    let seen = requested(&s.received_requests().await.unwrap());
    assert!(!seen.iter().any(|p| p.starts_with("/vanilla/")), "{seen:?}");
}

#[tokio::test]
async fn loader_errors_come_before_vanilla_downloads() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_modern_vanilla(&s).await;
    mount(&s, "/fabric/v2/versions/loader/9.9", b"[]".to_vec()).await;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let err = prepare(
        &paths,
        &opts("9.9", Some((LoaderKind::Fabric, None))),
        &endpoints(&u),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, Error::NoLoaderVersions { .. }), "{err}");
    assert!(!paths.libraries().exists());
    let seen = requested(&s.received_requests().await.unwrap());
    assert!(!seen.contains(&"/client.jar".to_string()), "{seen:?}");
}

#[tokio::test]
async fn unverifiable_loader_library_aborts_before_any_jar_is_fetched() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_modern_vanilla(&s).await;
    let (profile, jars) = mock_profile(FABRIC_PROFILE, "fabric-loader-0.20.1-9.9", "9.9", &u);
    // No inline hash and no sidecar (404) for fabric-loader.
    mount_jars(&s, &jars, &["fabric-loader"]).await;
    mount_fabric(&s, &profile).await;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let err = prepare(
        &paths,
        &opts("9.9", Some((LoaderKind::Fabric, None))),
        &endpoints(&u),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, Error::NoChecksum { url } if url.ends_with("fabric-loader-0.17.3.jar")),
        "{err}"
    );
    let seen = requested(&s.received_requests().await.unwrap());
    assert!(
        !seen.iter().any(|p| p.ends_with(".jar")),
        "a jar was requested: {seen:?}"
    );
}

#[tokio::test]
async fn profile_with_escaping_id_is_refused() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_modern_vanilla(&s).await;
    let (profile, _) = mock_profile(FABRIC_PROFILE, "../../evil", "9.9", &u);
    mount_fabric(&s, &profile).await;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let err = prepare(
        &paths,
        &opts("9.9", Some((LoaderKind::Fabric, None))),
        &endpoints(&u),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, Error::UnsafePath(_)), "{err}");
    assert!(!paths.libraries().exists());
    assert!(!paths.client_jar("9.9").exists());
}

#[tokio::test]
async fn forge_family_is_unsupported_for_now() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_modern_vanilla(&s).await;
    for kind in [LoaderKind::Forge, LoaderKind::NeoForge] {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let err = prepare(&paths, &opts("9.9", Some((kind, None))), &endpoints(&u))
            .await
            .unwrap_err();
        assert!(
            matches!(&err, Error::LoaderUnsupported { reason, .. } if reason == "not implemented yet"),
            "{err}"
        );
        assert!(!paths.libraries().exists());
    }
}

#[tokio::test]
async fn vanilla_launch_is_unchanged_without_a_loader() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_modern_vanilla(&s).await;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let p = prepare(&paths, &opts("9.9", None), &endpoints(&u))
        .await
        .unwrap();

    assert!(p.loader.is_none());
    assert_eq!(p.version_id, "9.9");
    assert_eq!(p.game_dir, paths.instance("9.9"));
    let natives = p.game_dir.join(".natives").join("9.9");
    let cp = format!(
        "{};{}",
        paths
            .libraries()
            .join("lwjgl-natives.jar")
            .to_string_lossy(),
        paths.client_jar("9.9").to_string_lossy()
    );
    assert_eq!(
        p.args,
        vec![
            "-Xmx1024M".to_string(),
            "-Xms512M".into(),
            format!("-Djava.library.path={}", natives.to_string_lossy()),
            "-cp".into(),
            cp,
            "net.minecraft.client.main.Main".into(),
            "--username".into(),
            "Dev".into(),
        ]
    );
}
