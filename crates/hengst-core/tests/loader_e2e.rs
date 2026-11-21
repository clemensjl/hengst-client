#![cfg(windows)]

use hengst_core::args::Account;
use hengst_core::download::sha1_hex;
use hengst_core::launch::{prepare, prepare_with_runner, Endpoints, LaunchOptions};
use hengst_core::libraries::maven_path;
use hengst_core::loaders::forge;
use hengst_core::loaders::forge::processors::ProcessRunner;
use hengst_core::loaders::{LoaderEndpoints, LoaderKind, LoaderRequest};
use hengst_core::paths::Paths;
use hengst_core::Error;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    mount_modern_vanilla_as(s, "9.9").await;
}

/// Like `mount_modern_vanilla`, under another version id.
async fn mount_modern_vanilla_as(s: &MockServer, id: &str) {
    let u = s.uri();
    let jar = zip_with("lwjgl.dll", b"dll");
    let asset_hash = sha1_hex(b"snd");
    let index = serde_json::to_vec(&json!({"objects": {"x.ogg": {"hash": asset_hash, "size": 3}}}))
        .unwrap();
    let runtime_manifest = serde_json::to_vec(&json!({"files": {
        "bin/java.exe": {"type": "file", "downloads": {"raw": {"sha1": sha1_hex(b"exe"), "size": 3, "url": format!("{u}/java.exe")}}}
    }})).unwrap();
    let version = serde_json::to_vec(&json!({
        "id": id, "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]},
        "assetIndex": {"id": "9", "sha1": sha1_hex(&index), "size": index.len(), "totalSize": 3, "url": format!("{u}/index.json")},
        "downloads": {"client": {"sha1": sha1_hex(b"client"), "size": 6, "url": format!("{u}/client.jar")}},
        "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
        "libraries": [{"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
            "downloads": {"artifact": {"path": "lwjgl-natives.jar", "sha1": sha1_hex(&jar), "size": jar.len(), "url": format!("{u}/lwjgl-natives.jar")}}}]
    })).unwrap();
    let runtimes = json!({"windows-x64": {"java-runtime-delta": [{"manifest": {
        "sha1": sha1_hex(&runtime_manifest), "size": runtime_manifest.len(), "url": format!("{u}/runtime.json")}}]}});

    mount_manifest(s, id, version).await;
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

// --- Forge and NeoForge ----------------------------------------------------------------------

const FORGE_1201_PROFILE: &str = include_str!("fixtures/loaders/forge-1.20.1-install_profile.json");
const FORGE_1201_VERSION: &str = include_str!("fixtures/loaders/forge-1.20.1-version.json");
const NEO_PROFILE: &str = include_str!("fixtures/loaders/neoforge-26.3-install_profile.json");
const FORGE_189_PROFILE: &str = include_str!("fixtures/loaders/forge-1.8.9-install_profile.json");
const FORGE_2859_PROFILE: &str =
    include_str!("fixtures/loaders/forge-1.12.2-2859-install_profile.json");
const FORGE_2859_VERSION: &str = include_str!("fixtures/loaders/forge-1.12.2-2859-version.json");

#[derive(Debug, Clone)]
struct Call {
    program: PathBuf,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    envs: Vec<(String, Option<String>)>,
}

/// Stands in for the JVM: records each command and writes the files the real processor would
/// (the value after `--output`, `--slim` and `--extra`) with `made <file name>` as content.
#[derive(Default)]
struct FakeJava {
    calls: Mutex<Vec<Call>>,
}

impl FakeJava {
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
}

fn made(file: &str) -> Vec<u8> {
    format!("made {file}").into_bytes()
}

impl ProcessRunner for FakeJava {
    fn run(&self, cmd: &mut Command, _: Duration) -> std::io::Result<Option<i32>> {
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        for w in args.windows(2) {
            if matches!(w[0].as_str(), "--output" | "--slim" | "--extra") {
                let out = Path::new(&w[1]);
                std::fs::create_dir_all(out.parent().unwrap())?;
                std::fs::write(out, made(&out.file_name().unwrap().to_string_lossy()))?;
            }
        }
        self.calls.lock().unwrap().push(Call {
            program: PathBuf::from(cmd.get_program()),
            args,
            cwd: cmd.get_current_dir().map(Path::to_path_buf),
            envs: cmd
                .get_envs()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.map(|v| v.to_string_lossy().into_owned()),
                    )
                })
                .collect(),
        });
        Ok(Some(0))
    }
}

/// Fails the test if any process would be started.
struct NoStart;

impl ProcessRunner for NoStart {
    fn run(&self, cmd: &mut Command, _: Duration) -> std::io::Result<Option<i32>> {
        panic!(
            "no processor may run: {:?}",
            cmd.get_args().collect::<Vec<_>>()
        );
    }
}

fn forge_endpoints(u: &str) -> Endpoints {
    let mut ep = endpoints(u);
    ep.loaders.forge_files = format!("{u}/forgefiles");
    ep.loaders.forge_maven = format!("{u}/forgemaven");
    ep.loaders.neoforge_maven = format!("{u}/neo");
    ep
}

fn jar_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        for (n, data) in entries {
            zip.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    buf.into_inner()
}

/// A small jar whose manifest names `test.<artifact>` as main class.
fn tool_jar(name: &str) -> Vec<u8> {
    let artifact = name.split(':').nth(1).unwrap().replace('-', "_");
    let mf = format!("Manifest-Version: 1.0\r\nMain-Class: test.{artifact}\r\n\r\n");
    jar_bytes(&[
        ("META-INF/MANIFEST.MF", mf.as_bytes()),
        ("name.txt", name.as_bytes()),
    ])
}

/// Points every downloadable artifact of `libs` at `{u}/m/<path>` with a synthetic jar and its
/// real SHA-1. Installer-provided ones (empty url) are left alone. Returns what to serve.
fn mock_artifacts(libs: &mut Value, u: &str) -> Vec<(String, Vec<u8>)> {
    let mut served = Vec::new();
    for lib in libs.as_array_mut().unwrap() {
        let name = lib["name"].as_str().unwrap().to_string();
        let Some(a) = lib.pointer_mut("/downloads/artifact") else {
            continue;
        };
        if a["url"].as_str() == Some("") {
            continue;
        }
        let rel = a["path"].as_str().unwrap().to_string();
        let body = tool_jar(&name);
        a["url"] = json!(format!("{u}/m/{rel}"));
        a["sha1"] = json!(sha1_hex(&body));
        a["size"] = json!(body.len());
        served.push((rel, body));
    }
    served
}

async fn serve_all(s: &MockServer, served: &[(String, Vec<u8>)]) {
    for (rel, body) in served {
        mount(s, &format!("/m/{rel}"), body.clone()).await;
    }
}

/// Serves an installer jar at `route` with its `.sha1`.
async fn serve_installer(s: &MockServer, route: &str, entries: &[(&str, &[u8])]) -> Vec<u8> {
    let jar = jar_bytes(entries);
    mount(s, route, jar.clone()).await;
    mount(s, &format!("{route}.sha1"), sha1_hex(&jar).into_bytes()).await;
    jar
}

async fn serve_forge_list(s: &MockServer, game: &str, maven: &str, recommended: Option<&str>) {
    mount(
        s,
        "/forgefiles/maven-metadata.json",
        serde_json::to_vec(&json!({ game: [maven] })).unwrap(),
    )
    .await;
    let promos = match recommended {
        Some(r) => json!({"promos": { format!("{game}-recommended"): r }}),
        None => json!({"promos": {}}),
    };
    mount(
        s,
        "/forgefiles/promotions_slim.json",
        serde_json::to_vec(&promos).unwrap(),
    )
    .await;
}

fn removed_env(call: &Call) -> Vec<String> {
    let mut v: Vec<String> = call
        .envs
        .iter()
        .filter(|(_, v)| v.is_none())
        .map(|(k, _)| k.to_ascii_uppercase())
        .collect();
    v.sort();
    v
}

fn main_class(call: &Call) -> &str {
    &call.args[2]
}

const FORGE_ID: &str = "1.20.1-forge-47.4.28";
const FORGE_PROVIDED: &str = "net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28.jar";

/// Forge 1.20.1 from the real profile and version fixtures: every library served by the mock,
/// the output hashes replaced by those of what `FakeJava` writes, and one installer-provided jar.
async fn mount_forge_1201(s: &MockServer) {
    let u = s.uri();
    mount_modern_vanilla_as(s, "1.20.1").await;
    serve_forge_list(s, "1.20.1", "1.20.1-47.4.28", Some("47.4.28")).await;
    let mut profile: Value = serde_json::from_str(FORGE_1201_PROFILE).unwrap();
    let mut served = mock_artifacts(&mut profile["libraries"], &u);
    let mc = "client-1.20.1-20230612.114412";
    for (key, file) in [
        ("MC_SLIM_SHA", format!("{mc}-slim.jar")),
        ("MC_EXTRA_SHA", format!("{mc}-extra.jar")),
        ("PATCHED_SHA", "forge-1.20.1-47.4.28-client.jar".to_string()),
    ] {
        profile["data"][key]["client"] = json!(format!("'{}'", sha1_hex(&made(&file))));
    }
    let mut version: Value = serde_json::from_str(FORGE_1201_VERSION).unwrap();
    served.extend(mock_artifacts(&mut version["libraries"], &u));
    version["libraries"].as_array_mut().unwrap().push(json!({
        "name": "net.minecraftforge:forge:1.20.1-47.4.28",
        "downloads": {"artifact": {"path": FORGE_PROVIDED, "url": "",
            "sha1": sha1_hex(b"provided forge"), "size": 14}}
    }));
    serve_all(s, &served).await;
    serve_installer(
        s,
        "/forgemaven/net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-installer.jar",
        &[
            ("install_profile.json", profile.to_string().as_bytes()),
            ("version.json", version.to_string().as_bytes()),
            ("data/client.lzma", b"client patches"),
            (&format!("maven/{FORGE_PROVIDED}"), b"provided forge"),
        ],
    )
    .await;
}

#[tokio::test]
async fn prepares_modern_forge_running_its_processors_once() {
    let s = MockServer::start().await;
    let u = s.uri();
    mount_forge_1201(&s).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut o = opts("1.20.1", Some((LoaderKind::Forge, None)));
    o.account = Account::offline("ProbeAccountName");
    let uuid = o.account.uuid().to_string();
    let fake = Arc::new(FakeJava::default());
    let ep = forge_endpoints(&u);

    let p = prepare_with_runner(&paths, &o, &ep, fake.clone())
        .await
        .unwrap();
    assert_eq!(p.version_id, FORGE_ID);
    assert_eq!(p.game_dir, paths.instance("1.20.1-forge"));
    assert!(p
        .args
        .contains(&"cpw.mods.bootstraplauncher.BootstrapLauncher".to_string()));
    let cp = classpath(&p.args);
    assert!(cp.contains("forge-1.20.1-47.4.28.jar"), "{cp}");

    let calls = fake.calls();
    let mains: Vec<&str> = calls.iter().map(main_class).collect();
    assert_eq!(
        mains,
        [
            "test.installertools",
            "test.installertools",
            "test.installertools",
            "test.jarsplitter",
            "test.ForgeAutoRenamingTool",
            "test.binarypatcher"
        ]
    );
    assert_eq!(calls[0].args[3..5], ["--task", "MCP_DATA"]);
    assert_eq!(calls[1].args[3..5], ["--task", "DOWNLOAD_MOJMAPS"]);
    let java = paths
        .java_runtime("java-runtime-delta")
        .join("bin")
        .join("java.exe");
    let work = std::path::absolute(paths.tmp())
        .unwrap()
        .join(format!("loader-{FORGE_ID}"));
    for c in &calls {
        assert_eq!(c.program, java);
        assert_eq!(c.args[0], "-cp");
        assert_eq!(c.cwd.as_deref(), Some(work.as_path()));
        assert_eq!(
            removed_env(c),
            [
                "CLASSPATH",
                "JAVA_TOOL_OPTIONS",
                "JDK_JAVA_OPTIONS",
                "_JAVA_OPTIONS"
            ]
        );
        assert!(c.envs.iter().all(|(_, v)| v.is_none()), "{c:?}");
        // No account data reaches a processor.
        assert!(!c
            .args
            .iter()
            .any(|a| a.contains("ProbeAccountName") || a.contains(&uuid)));
    }
    let marker = forge::marker_path(&paths, LoaderKind::Forge, FORGE_ID);
    assert!(marker.is_file());
    assert!(!work.exists());
    let patched = paths
        .libraries()
        .join("net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-client.jar");
    assert_eq!(
        std::fs::read(&patched).unwrap(),
        made("forge-1.20.1-47.4.28-client.jar")
    );
    assert!(forge::install_log(&paths, FORGE_ID).is_file());

    // Second launch: valid state, no processor runs.
    prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap();

    // A produced file without a declared hash changes: everything runs again.
    let mappings = paths.libraries().join("de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412-mappings.txt");
    assert!(mappings.is_file());
    std::fs::write(&mappings, b"tampered").unwrap();
    prepare_with_runner(&paths, &o, &ep, fake.clone())
        .await
        .unwrap();
    assert_eq!(fake.calls().len(), 12);

    // A foreign file at the installer-provided path is replaced, without a processor run.
    let provided = paths.libraries().join(FORGE_PROVIDED);
    std::fs::write(&provided, b"someone else's forge").unwrap();
    prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&provided).unwrap(), b"provided forge");
}

/// NeoForge from the real 26.3 profile, or the same profile rewritten for 1.20.1, where NeoForge
/// still publishes under `net.neoforged:forge`.
async fn mount_neoforge(s: &MockServer, game: &str, maven: &str, artifact: &str, id: &str) {
    let u = s.uri();
    mount_modern_vanilla_as(s, game).await;
    let listed = if game == "1.20.1" {
        "forge"
    } else {
        "neoforge"
    };
    mount(
        s,
        &format!("/neo/api/maven/versions/releases/net/neoforged/{listed}"),
        serde_json::to_vec(&json!({ "versions": [maven] })).unwrap(),
    )
    .await;
    let text = NEO_PROFILE
        .replace(
            "\"minecraft\": \"26.3\"",
            &format!("\"minecraft\": \"{game}\""),
        )
        .replace("26.3.0.115", maven);
    let mut profile: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(profile["minecraft"], game);
    let mut served = mock_artifacts(&mut profile["libraries"], &u);
    let provided = format!("net/neoforged/{artifact}/{maven}/{artifact}-{maven}.jar");
    let mut version = json!({
        "id": id, "inheritsFrom": game, "mainClass": "net.neoforged.fml.startup.Client",
        "arguments": {"game": ["--fml.neoForgeVersion", maven], "jvm": []},
        "libraries": [
            {"name": "net.neoforged:bus:8.0.5", "downloads": {"artifact": {
                "path": "net/neoforged/bus/8.0.5/bus-8.0.5.jar", "url": "x", "sha1": "", "size": 0}}},
            {"name": format!("net.neoforged:{artifact}:{maven}"), "downloads": {"artifact": {
                "path": provided, "url": "", "sha1": sha1_hex(b"provided neo"), "size": 12}}}
        ]
    });
    served.extend(mock_artifacts(&mut version["libraries"], &u));
    serve_all(s, &served).await;
    serve_installer(
        s,
        &format!("/neo/releases/net/neoforged/{artifact}/{maven}/{artifact}-{maven}-installer.jar"),
        &[
            ("install_profile.json", profile.to_string().as_bytes()),
            ("version.json", version.to_string().as_bytes()),
            ("data/client.lzma", b"client patches"),
            (&format!("maven/{provided}"), b"provided neo"),
        ],
    )
    .await;
}

async fn check_neoforge(game: &str, maven: &str, artifact: &str, id: &str) {
    let s = MockServer::start().await;
    mount_neoforge(&s, game, maven, artifact, id).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let o = opts(game, Some((LoaderKind::NeoForge, None)));
    let ep = forge_endpoints(&s.uri());
    let fake = Arc::new(FakeJava::default());

    let p = prepare_with_runner(&paths, &o, &ep, fake.clone())
        .await
        .unwrap();
    assert_eq!(p.version_id, id);
    assert!(p
        .args
        .contains(&"net.neoforged.fml.startup.Client".to_string()));
    let calls = fake.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let args = &calls[0].args;
    assert_eq!(main_class(&calls[0]), "test.installertools");
    assert_eq!(args[3..5], ["--task", "PROCESS_MINECRAFT_JAR"]);
    let root = std::path::absolute(paths.root()).unwrap();
    assert!(
        args.contains(&format!("{}/libraries/", root.display())),
        "{args:?}"
    );
    let vanilla = root.join("versions").join(game).join(format!("{game}.jar"));
    assert!(args.contains(&vanilla.display().to_string()), "{args:?}");
    let patched = paths.libraries().join(format!(
        "net/neoforged/minecraft-client-patched/{maven}/minecraft-client-patched-{maven}.jar"
    ));
    assert!(patched.is_file());
    assert!(forge::marker_path(&paths, LoaderKind::NeoForge, id).is_file());

    prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    // NeoForge declares no output hashes: only the marker protects the patched jar.
    std::fs::write(&patched, b"tampered").unwrap();
    prepare_with_runner(&paths, &o, &ep, fake.clone())
        .await
        .unwrap();
    assert_eq!(fake.calls().len(), 2);
}

#[tokio::test]
async fn prepares_neoforge_running_its_processor_once() {
    check_neoforge("26.3", "26.3.0.115", "neoforge", "neoforge-26.3.0.115").await;
}

#[tokio::test]
async fn prepares_neoforge_for_1_20_1_under_the_forge_artifact() {
    check_neoforge(
        "1.20.1",
        "1.20.1-47.1.106",
        "forge",
        "1.20.1-neoforge-47.1.106",
    )
    .await;
}

const LEGACY_MAVEN: &str = "8.8-11.15.1.2318-8.8";
const LEGACY_UNIVERSAL: &str =
    "net/minecraftforge/forge/8.8-11.15.1.2318-8.8/forge-8.8-11.15.1.2318-8.8.jar";

/// Legacy-format Forge (the 1.8.9 fixture) rewritten for vanilla "8.8": launchwrapper served with
/// a sidecar, akka with an inline checksum, jline client-excluded.
async fn mount_legacy_forge(s: &MockServer) {
    let u = s.uri();
    mount_legacy_vanilla(s).await;
    serve_forge_list(s, "8.8", LEGACY_MAVEN, Some("11.15.1.2318")).await;
    let mut profile: Value =
        serde_json::from_str(&FORGE_189_PROFILE.replace("1.8.9", "8.8")).unwrap();
    for lib in profile["versionInfo"]["libraries"].as_array_mut().unwrap() {
        let name = lib["name"].as_str().unwrap().to_string();
        if name.starts_with("net.minecraftforge:forge:") {
            continue;
        }
        let body = tool_jar(&name);
        let rel = maven_path(&name).unwrap();
        lib["url"] = json!(format!("{u}/m/"));
        if lib.get("checksums").is_some() {
            lib["checksums"] = json!([sha1_hex(&body)]);
        } else {
            mount(s, &format!("/m/{rel}.sha1"), sha1_hex(&body).into_bytes()).await;
        }
        mount(s, &format!("/m/{rel}"), body).await;
    }
    serve_installer(
        s,
        "/forgemaven/net/minecraftforge/forge/8.8-11.15.1.2318-8.8/forge-8.8-11.15.1.2318-8.8-installer.jar",
        &[
            ("install_profile.json", profile.to_string().as_bytes()),
            ("forge-8.8-11.15.1.2318-8.8-universal.jar", b"universal jar"),
        ],
    )
    .await;
}

#[tokio::test]
async fn prepares_legacy_format_forge() {
    let s = MockServer::start().await;
    mount_legacy_forge(&s).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut o = opts("8.8", Some((LoaderKind::Forge, None)));
    o.java = Some(PathBuf::from("java.exe"));
    let ep = forge_endpoints(&s.uri());

    let p = prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    assert_eq!(p.version_id, "8.8-forge8.8-11.15.1.2318-8.8");
    assert_eq!(p.game_dir, paths.instance("8.8-forge"));
    let main = p
        .args
        .iter()
        .position(|a| a == "net.minecraft.launchwrapper.Launch")
        .unwrap();
    let tweak = p.args.iter().position(|a| a == "--tweakClass").unwrap();
    assert!(main < tweak);
    assert_eq!(
        p.args[tweak + 1],
        "net.minecraftforge.fml.common.launcher.FMLTweaker"
    );
    let cp = classpath(&p.args);
    assert!(cp.contains("forge-8.8-11.15.1.2318-8.8.jar"), "{cp}");
    assert!(cp.contains("launchwrapper-1.12.jar"), "{cp}");
    assert!(cp.contains("akka-actor_2.11-2.3.3.jar"), "{cp}");
    assert!(!cp.contains("jline"), "{cp}");
    assert!(cp.ends_with(
        &paths
            .client_jar("8.8-forge8.8-11.15.1.2318-8.8")
            .to_string_lossy()
            .into_owned()
    ));
    let universal = paths.libraries().join(LEGACY_UNIVERSAL);
    assert_eq!(std::fs::read(&universal).unwrap(), b"universal jar");
    let seen = requested(&s.received_requests().await.unwrap());
    assert!(!seen.iter().any(|p| p.contains("jline")), "{seen:?}");
    assert!(
        !seen
            .iter()
            .any(|p| p.contains("forge-8.8-11.15.1.2318-8.8.jar")),
        "the universal jar comes from the installer: {seen:?}"
    );

    {
        // A running game holds the correct jar open without delete sharing: relaunching works
        // and does not try to replace it.
        use std::os::windows::fs::OpenOptionsExt;
        let _game = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&universal)
            .unwrap();
        prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
            .await
            .unwrap();
    }

    // A foreign file at the universal jar's path never reaches the classpath.
    std::fs::write(&universal, b"foreign").unwrap();
    prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&universal).unwrap(), b"universal jar");
}

const SPEC0_PROVIDED: &str = "net/minecraftforge/forge/8.8-14.23.5.2859/forge-8.8-14.23.5.2859.jar";

/// The 1.12.2-2859 pattern (modern format, spec 0, no processors) rewritten for vanilla "8.8".
async fn mount_spec0_forge(s: &MockServer, minecraft: &str) {
    let u = s.uri();
    mount_legacy_vanilla(s).await;
    serve_forge_list(s, "8.8", "8.8-14.23.5.2859", None).await;
    let real_sha = "bfc07a510e23e5181b6a32fdcea9c9d9a1445a46";
    let fix = |t: &str| {
        t.replace("1.12.2", "8.8")
            .replace(real_sha, &sha1_hex(b"forge jar"))
    };
    let mut profile: Value = serde_json::from_str(&fix(FORGE_2859_PROFILE)).unwrap();
    profile["minecraft"] = json!(minecraft);
    let mut served = mock_artifacts(&mut profile["libraries"], &u);
    let mut version: Value = serde_json::from_str(&fix(FORGE_2859_VERSION)).unwrap();
    served.extend(mock_artifacts(&mut version["libraries"], &u));
    serve_all(s, &served).await;
    serve_installer(
        s,
        "/forgemaven/net/minecraftforge/forge/8.8-14.23.5.2859/forge-8.8-14.23.5.2859-installer.jar",
        &[
            ("install_profile.json", profile.to_string().as_bytes()),
            ("version.json", version.to_string().as_bytes()),
            (&format!("maven/{SPEC0_PROVIDED}"), b"forge jar"),
        ],
    )
    .await;
}

#[tokio::test]
async fn prepares_spec0_forge_without_processors() {
    let s = MockServer::start().await;
    mount_spec0_forge(&s, "8.8").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut o = opts("8.8", Some((LoaderKind::Forge, None)));
    o.java = Some(PathBuf::from("java.exe"));
    let ep = forge_endpoints(&s.uri());

    let p = prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    assert_eq!(p.version_id, "8.8-forge-14.23.5.2859");
    assert!(p.args.iter().any(|a| a.contains("--versionType")));
    assert!(p
        .args
        .contains(&"net.minecraft.launchwrapper.Launch".to_string()));
    let cp = classpath(&p.args);
    assert!(cp.contains("forge-8.8-14.23.5.2859.jar"), "{cp}");
    let provided = paths.libraries().join(SPEC0_PROVIDED);
    assert_eq!(std::fs::read(&provided).unwrap(), b"forge jar");
    // No processor stage, so no marker.
    assert!(!forge::marker_path(&paths, LoaderKind::Forge, &p.version_id).exists());

    std::fs::write(&provided, b"foreign").unwrap();
    prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&provided).unwrap(), b"forge jar");
}

#[tokio::test]
async fn forge_installer_for_wrong_game_version_is_refused() {
    let s = MockServer::start().await;
    mount_spec0_forge(&s, "8.7").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let err = prepare_with_runner(
        &paths,
        &opts("8.8", Some((LoaderKind::Forge, None))),
        &forge_endpoints(&s.uri()),
        Arc::new(NoStart),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, Error::ParentMismatch { expected, found } if expected == "8.8" && found == "8.7"),
        "{err}"
    );
    let seen = requested(&s.received_requests().await.unwrap());
    assert!(!seen.iter().any(|p| p.starts_with("/m/")), "{seen:?}");
    assert!(!seen.contains(&"/client.jar".to_string()), "{seen:?}");
}

#[tokio::test]
async fn installed_forge_launches_offline_and_fails_closed_on_a_damaged_file() {
    // Not from the pool: dropping it really stops the server.
    let s = MockServer::builder().start().await;
    let u = s.uri();
    mount_forge_1201(&s).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let ep = forge_endpoints(&u);
    let default = opts("1.20.1", Some((LoaderKind::Forge, None)));
    let online = prepare_with_runner(&paths, &default, &ep, Arc::new(FakeJava::default()))
        .await
        .unwrap();
    assert!(!online.loader_list_from_cache);
    drop(s);
    assert!(reqwest::get(format!("{u}/forgefiles/maven-metadata.json"))
        .await
        .is_err());

    // Default build: the saved version list stands in for the unreachable service.
    let p = prepare_with_runner(&paths, &default, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    assert!(p.loader_list_from_cache);
    assert_eq!(p.version_id, FORGE_ID);
    assert_eq!(p.args, online.args);
    // A named build needs no list at all.
    let named = opts("1.20.1", Some((LoaderKind::Forge, Some("47.4.28"))));
    let p = prepare_with_runner(&paths, &named, &ep, Arc::new(NoStart))
        .await
        .unwrap();
    assert!(!p.loader_list_from_cache);
    assert_eq!(p.version_id, FORGE_ID);

    // A damaged library is not trusted and cannot be fetched again: a clear offline error.
    let lib = paths
        .libraries()
        .join("cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar");
    std::fs::write(&lib, b"damaged").unwrap();
    let err = prepare_with_runner(&paths, &named, &ep, Arc::new(NoStart))
        .await
        .unwrap_err();
    let m = err.to_string();
    assert!(matches!(err, Error::Offline { .. }), "{m}");
    assert!(
        m.contains("securejarhandler-2.1.10.jar") && m.contains("offline"),
        "{m}"
    );
}

#[tokio::test]
async fn named_forge_build_skips_the_version_list() {
    let s = MockServer::start().await;
    mount_forge_1201(&s).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let ep = forge_endpoints(&s.uri());
    for requested in ["47.4.28", "1.20.1-47.4.28"] {
        let o = opts("1.20.1", Some((LoaderKind::Forge, Some(requested))));
        let p = prepare_with_runner(&paths, &o, &ep, Arc::new(FakeJava::default()))
            .await
            .unwrap();
        assert_eq!(p.version_id, FORGE_ID);
        assert_eq!(p.loader.unwrap().version.maven, "1.20.1-47.4.28");
    }
    let seen = requested(&s.received_requests().await.unwrap());
    assert!(
        !seen.iter().any(|p| p.starts_with("/forgefiles/")),
        "{seen:?}"
    );
    // A build that does not exist surfaces as an unknown version (no installer).
    let o = opts("1.20.1", Some((LoaderKind::Forge, Some("47.0.0"))));
    let err = prepare_with_runner(&paths, &o, &ep, Arc::new(NoStart))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::UnknownLoaderVersion { .. }), "{err}");
}
