use super::*;
use crate::dto::instances::{LoaderKindDto, LoaderRequestDto};
use hengst_core::launch::Endpoints;
use hengst_core::paths::Paths;
use hengst_core::Result as CoreResult;
use hengst_testkit::{endpoints, mount_mojang};
use std::path::{Path, PathBuf};
use std::time::Duration;
use wiremock::MockServer;

const FAKE: &str = "FAKE-ACCESS-TOKEN-7f3a";

type Log = Arc<Mutex<Vec<AppEvent>>>;

struct Fixture {
    server: MockServer,
    dir: tempfile::TempDir,
    state: AppState,
    log: Log,
}

impl Fixture {
    fn paths(&self) -> &Paths {
        &self.state.paths
    }

    fn events(&self) -> Vec<AppEvent> {
        self.log.lock().unwrap().clone()
    }

    fn clear_events(&self) {
        self.log.lock().unwrap().clear();
    }
}

async fn fixture() -> Fixture {
    let server = MockServer::builder().start().await;
    mount_mojang(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::new(
        Paths::new(dir.path().join("home")),
        endpoints(&server.uri()),
    );
    // No test can reach the real Recycle Bin: a test that deletes sets its own trasher.
    state.trasher = Arc::new(|_| -> CoreResult<()> {
        Err(Error::Io(std::io::Error::other(
            "the test fixture has no Recycle Bin",
        )))
    });
    let log: Log = Arc::default();
    let sink = log.clone();
    state
        .events
        .subscribe(Arc::new(move |e| sink.lock().unwrap().push(e)));
    Fixture {
        server,
        dir,
        state,
        log,
    }
}

fn instances_changed() -> AppEvent {
    AppEvent::DataChanged {
        scope: DataScope::Instances,
    }
}

fn settings_changed() -> AppEvent {
    AppEvent::DataChanged {
        scope: DataScope::Settings,
    }
}

fn request(
    name: &str,
    game: &str,
    loader: Option<(LoaderKindDto, Option<&str>)>,
) -> CreateInstanceRequest {
    CreateInstanceRequest {
        name: name.into(),
        game: game.into(),
        loader: loader.map(|(kind, version)| LoaderRequestDto {
            kind,
            version: version.map(Into::into),
        }),
    }
}

/// A plain instance on disk, made by the core.
fn make(fx: &Fixture, name: &str, game: &str) -> instances::Instance {
    instances::create(
        fx.paths(),
        &NewInstance {
            name: name.into(),
            game: game.into(),
            loader: None,
        },
        1,
    )
    .unwrap()
}

fn write_world(dir: &Path) -> PathBuf {
    let level = dir.join("saves/world/level.dat");
    std::fs::create_dir_all(level.parent().unwrap()).unwrap();
    std::fs::write(&level, b"world").unwrap();
    std::fs::write(dir.join("options.txt"), b"o").unwrap();
    level
}

fn select(fx: &Fixture, id: Option<&str>) {
    fx.state
        .settings
        .update(|s| {
            s.selected_instance = id.map(Into::into);
            Ok(())
        })
        .unwrap();
}

fn selected(fx: &Fixture) -> Option<String> {
    fx.state.settings.get().selected_instance
}

/// A trasher that moves the folder into `<home>/../bin`.
fn bin_trasher(fx: &Fixture) -> (crate::state::Trasher, PathBuf) {
    let bin = fx.dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let to = bin.clone();
    let trasher: crate::state::Trasher = Arc::new(move |p| {
        std::fs::rename(p, to.join(p.file_name().unwrap()))?;
        Ok(())
    });
    (trasher, bin)
}

fn failing_trasher(calls: Arc<Mutex<u32>>) -> crate::state::Trasher {
    Arc::new(move |_| -> CoreResult<()> {
        *calls.lock().unwrap() += 1;
        Err(Error::Io(std::io::Error::other("the bin refused")))
    })
}

// Loader kinds and versions

#[test]
fn loader_kinds_carry_display_names() {
    let kinds = loader_kinds_impl();
    let pairs: Vec<(String, &str)> = kinds
        .iter()
        .map(|k| {
            (
                serde_json::to_string(&k.kind).unwrap(),
                k.display_name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        [
            ("\"fabric\"".to_string(), "Fabric"),
            ("\"quilt\"".to_string(), "Quilt"),
            ("\"legacy-fabric\"".to_string(), "Legacy Fabric"),
            ("\"forge\"".to_string(), "Forge"),
            ("\"neoforge\"".to_string(), "NeoForge"),
        ]
    );
}

#[test]
fn the_loader_dto_maps_to_the_core_kind_and_back() {
    for dto in LOADER_ORDER {
        let kind: LoaderKind = dto.into();
        assert_eq!(LoaderKindDto::from(kind), dto);
        assert_eq!(
            serde_json::to_string(&dto).unwrap(),
            format!("\"{}\"", kind.slug())
        );
    }
    assert!(serde_json::from_str::<LoaderKindDto>("\"Fabric\"").is_err());
    assert!(serde_json::from_str::<LoaderKindDto>("\"fabric \"").is_err());
}

#[tokio::test]
async fn loader_versions_mark_the_recommended_build() {
    let fx = fixture().await;
    let r = loader_versions_impl(&fx.state, LoaderKindDto::Fabric, "9.9")
        .await
        .unwrap();
    assert_eq!(r.recommended, "0.99.0");
    let beta = r
        .versions
        .iter()
        .find(|v| v.id == "0.100.0-beta.1")
        .unwrap();
    assert!(!beta.stable);
    assert!(r.versions.iter().any(|v| v.id == "0.99.0" && v.stable));
}

#[tokio::test]
async fn loader_versions_for_an_unknown_game_ask_nobody_but_the_version_list() {
    let fx = fixture().await;
    let dto = loader_versions_impl(&fx.state, LoaderKindDto::Fabric, "8.8")
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::UnknownVersion);
    let asked: Vec<String> = fx
        .server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    assert!(asked.iter().all(|p| !p.starts_with("/fabric")), "{asked:?}");
}

// Create

#[tokio::test]
async fn create_validates_game_and_loader_version() {
    let fx = fixture().await;
    let ok = instance_create_impl(
        &fx.state,
        request("Mira", "9.9", Some((LoaderKindDto::Fabric, Some("0.99.0")))),
    )
    .await
    .unwrap();
    assert_eq!(ok.id, "9.9-fabric");
    assert!(ok.selected);
    assert_eq!(selected(&fx).as_deref(), Some("9.9-fabric"));

    let r = instance_create_impl(
        &fx.state,
        request("x", "9.9", Some((LoaderKindDto::Fabric, Some("0.0.1")))),
    )
    .await;
    assert_eq!(r.unwrap_err().code, ErrorCode::UnknownLoaderVersion);
    let r = instance_create_impl(&fx.state, request("x", "8.8", None)).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::UnknownVersion);
    let r = instance_create_impl(
        &fx.state,
        request("x", "b9.9", Some((LoaderKindDto::Fabric, None))),
    )
    .await;
    assert_eq!(r.unwrap_err().code, ErrorCode::NoLoaderVersions);
    // Only the first request made anything.
    assert_eq!(instances::list(fx.paths()).unwrap().items.len(), 1);
}

#[tokio::test]
async fn create_without_a_loader_version_saves_the_recommended_build() {
    let fx = fixture().await;
    let dto = instance_create_impl(
        &fx.state,
        request("Mira", "9.9", Some((LoaderKindDto::Fabric, None))),
    )
    .await
    .unwrap();
    assert_eq!(dto.loader.unwrap().version.as_deref(), Some("0.99.0"));
    let on_disk = instances::load(fx.paths(), "9.9-fabric").unwrap();
    assert_eq!(
        on_disk.config.loader.unwrap().version.as_deref(),
        Some("0.99.0")
    );
}

#[tokio::test]
async fn created_instance_dto_has_slug_and_display_name() {
    let fx = fixture().await;
    let dto = instance_create_impl(
        &fx.state,
        request("Mira", "9.9", Some((LoaderKindDto::Fabric, Some("0.99.0")))),
    )
    .await
    .unwrap();
    let json = serde_json::to_value(&dto).unwrap();
    assert_eq!(json["loader"]["kind"], "fabric");
    assert_eq!(json["loader"]["displayName"], "Fabric");
    assert_eq!(json["loader"]["version"], "0.99.0");
    assert_eq!(json["status"], "ready");
    assert_eq!(json["name"], "Mira");
    assert_eq!(json["game"], "9.9");
    assert_eq!(json["selected"], true);
}

#[tokio::test]
async fn create_emits_instances_and_settings_changes() {
    let fx = fixture().await;
    instance_create_impl(&fx.state, request("A", "9.9", None))
        .await
        .unwrap();
    let events = fx.events();
    assert!(events.contains(&instances_changed()), "{events:?}");
    assert!(events.contains(&settings_changed()), "{events:?}");
}

#[tokio::test]
async fn a_refused_create_changes_nothing_and_sends_no_event() {
    let fx = fixture().await;
    let r = instance_create_impl(&fx.state, request("   ", "9.9", None)).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::InvalidInstanceName);
    let r = instance_create_impl(&fx.state, request("A", "8.8", None)).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::UnknownVersion);
    assert!(fx.events().is_empty(), "{:?}", fx.events());
    assert!(!fx.paths().root().join("instances").exists());
}

#[tokio::test]
async fn an_invalid_name_is_refused_before_any_request() {
    let fx = fixture().await;
    let r = instance_create_impl(&fx.state, request("a\nb", "9.9", None)).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::InvalidInstanceName);
    assert!(fx.server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn create_without_a_loader_asks_only_for_the_version_list() {
    let fx = fixture().await;
    instance_create_impl(&fx.state, request("A", "9.9", None))
        .await
        .unwrap();
    let asked: Vec<String> = fx
        .server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    assert_eq!(asked, ["/mojang/manifest.json"]);
    // With the list saved a second instance needs no request at all.
    instance_create_impl(&fx.state, request("B", "9.9", None))
        .await
        .unwrap();
    assert_eq!(fx.server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn create_and_delete_are_serialized() {
    let fx = fixture().await;
    let a = request("A", "9.9", Some((LoaderKindDto::Fabric, Some("0.99.0"))));
    let b = request("B", "9.9", Some((LoaderKindDto::Fabric, Some("0.99.0"))));
    let (first, second) = tokio::join!(
        instance_create_impl(&fx.state, a),
        instance_create_impl(&fx.state, b)
    );
    assert!(first.is_ok(), "{first:?}");
    assert_eq!(second.unwrap_err().code, ErrorCode::Busy);
    assert_eq!(instances::list(fx.paths()).unwrap().items.len(), 1);

    // A delete while a create runs is refused the same way, and the other way round.
    let id = first.unwrap().id;
    let (create, delete) = tokio::join!(
        instance_create_impl(&fx.state, request("C", "9.9", None)),
        instance_delete_impl(&fx.state, id.clone())
    );
    assert!(create.is_ok(), "{create:?}");
    assert_eq!(delete.unwrap_err().code, ErrorCode::Busy);
    assert!(
        fx.paths().instance(&id).is_dir(),
        "the refused delete touched nothing"
    );
}

// List

#[tokio::test]
async fn list_adopts_cli_folders_when_the_manifest_is_cached() {
    let fx = fixture().await;
    std::fs::create_dir_all(fx.paths().instance("9.9-fabric")).unwrap();
    std::fs::create_dir_all(fx.paths().instance("notes")).unwrap();
    // Without a saved version list nothing is adopted.
    assert!(instances_list_impl(&fx.state)
        .await
        .unwrap()
        .instances
        .is_empty());
    assert!(!fx
        .paths()
        .instance("9.9-fabric")
        .join("instance.json")
        .exists());
    assert!(fx.events().is_empty());
    // Save the list (a first game_versions call does), then adopt.
    game_versions_impl(
        &fx.state,
        VersionFilter {
            release: true,
            snapshot: false,
            beta: false,
            alpha: false,
        },
    )
    .await
    .unwrap();
    let list = instances_list_impl(&fx.state).await.unwrap().instances;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "9.9-fabric");
    assert_eq!(list[0].name, "9.9 Fabric");
    assert_eq!(list[0].loader.as_ref().unwrap().display_name, "Fabric");
    assert_eq!(list[0].loader.as_ref().unwrap().version, None);
    assert!(!fx.paths().instance("notes").join("instance.json").exists());
    assert!(fx.events().contains(&instances_changed()));
    // A second list adopts nothing and sends nothing.
    fx.clear_events();
    assert_eq!(
        instances_list_impl(&fx.state)
            .await
            .unwrap()
            .instances
            .len(),
        1
    );
    assert!(fx.events().is_empty());
}

#[tokio::test]
async fn list_shows_every_state_and_marks_the_selection() {
    let fx = fixture().await;
    make(&fx, "Ready one", "9.9");
    std::fs::create_dir_all(fx.paths().instance("old")).unwrap();
    std::fs::write(
        fx.paths().instance("old").join("instance.json"),
        r#"{"format":2}"#,
    )
    .unwrap();
    std::fs::create_dir_all(fx.paths().instance("bad")).unwrap();
    std::fs::write(
        fx.paths().instance("bad").join("instance.json"),
        format!("{{ \"x\": \"{FAKE}\""),
    )
    .unwrap();
    select(&fx, Some("old"));
    let dto = instances_list_impl(&fx.state).await.unwrap();
    // Every entry could be read, so the list says it is complete.
    assert!(!dto.incomplete);
    let list = dto.instances;
    let by_id = |id: &str| list.iter().find(|i| i.id == id).unwrap();
    assert_eq!(by_id("9.9").status, InstanceStatus::Ready);
    assert!(!by_id("9.9").selected);
    let old = by_id("old");
    assert_eq!(
        (old.status, old.name.as_str(), old.game.as_str()),
        (InstanceStatus::TooNew, "old", "")
    );
    assert!(old.selected);
    let bad = by_id("bad");
    assert_eq!(bad.status, InstanceStatus::Broken);
    // The reason is the core's fixed text; it quotes nothing from the file.
    let reason = bad.reason.as_deref().unwrap();
    assert!(reason.contains("not valid JSON"), "{reason}");
    assert!(!serde_json::to_string(&list).unwrap().contains(FAKE));
}

#[tokio::test]
async fn list_waits_for_create_and_never_adopts_a_half_made_folder() {
    let fx = Arc::new(fixture().await);
    let guard = fx.state.instance_lock.lock().await;
    let f = fx.clone();
    let task = tokio::spawn(async move { instances_list_impl(&f.state).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!task.is_finished(), "the list must wait for the lock");
    drop(guard);
    let list = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert!(list.is_ok());
}

#[test]
fn instance_dtos_carry_neither_the_path_nor_unknown_fields_nor_the_java_path() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("home"));
    let folder = paths.instance("9.9");
    std::fs::create_dir_all(&folder).unwrap();
    let java = "C:\\Java\\private-java-path\\bin\\javaw.exe";
    std::fs::write(
        folder.join("instance.json"),
        serde_json::json!({
            "format": 1, "name": "Mira", "game": "9.9", "loader": {"kind": "fabric", "version": "0.99.0", "token": FAKE},
            "memoryMb": 4096, "java": java, "createdAt": 1, "lastPlayed": 7,
            "notes": FAKE, "nested": {"deep": FAKE}
        })
        .to_string(),
    )
    .unwrap();
    let list = instances::list(&paths).unwrap();
    let dtos: Vec<InstanceDto> = list.items.iter().map(|l| listed_dto(l, None)).collect();
    let json = serde_json::to_string(&dtos).unwrap();
    assert!(
        json.contains("\"memoryMb\":4096") && json.contains("\"lastPlayed\":7"),
        "{json}"
    );
    for secret in [
        FAKE,
        java,
        "private-java-path",
        "notes",
        "nested",
        "createdAt",
    ] {
        assert!(!json.contains(secret), "{secret} leaked into {json}");
    }
    let root = dir.path().display().to_string();
    let escaped = root.replace('\\', "\\\\");
    assert!(
        !json.contains(&root) && !json.contains(&escaped),
        "the folder path leaked: {json}"
    );
    // The field names of a row, exactly.
    let mut keys: Vec<String> = serde_json::to_value(&dtos[0])
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "game",
            "id",
            "lastPlayed",
            "loader",
            "memoryMb",
            "name",
            "reason",
            "selected",
            "status"
        ]
    );
}

// Select

#[tokio::test]
async fn select_refuses_unknown_unreadable_and_unlisted_ids() {
    let fx = fixture().await;
    make(&fx, "A", "9.9");
    select(&fx, Some("9.9"));
    std::fs::create_dir_all(fx.paths().instance("9.9-fabric").join("saves")).unwrap();
    for (id, json) in [("broken", "{"), ("newer", r#"{"format":2}"#)] {
        std::fs::create_dir_all(fx.paths().instance(id)).unwrap();
        std::fs::write(fx.paths().instance(id).join("instance.json"), json).unwrap();
    }
    fx.clear_events();
    for (id, code) in [
        ("fehlt", ErrorCode::InstanceNotFound),
        ("..\\x", ErrorCode::InstanceNotFound),
        ("a/b", ErrorCode::InstanceNotFound),
        ("", ErrorCode::InstanceNotFound),
        ("9.9-fabric", ErrorCode::InstanceNotFound),
        ("broken", ErrorCode::InstanceUnreadable),
        ("newer", ErrorCode::InstanceFormatTooNew),
    ] {
        let dto = instance_select_impl(&fx.state, id.to_string())
            .await
            .unwrap_err();
        assert_eq!(dto.code, code, "{id:?}");
    }
    assert_eq!(selected(&fx).as_deref(), Some("9.9"));
    assert!(fx.events().is_empty());
    make(&fx, "B", "1.0");
    instance_select_impl(&fx.state, "1.0".into()).await.unwrap();
    assert_eq!(selected(&fx).as_deref(), Some("1.0"));
    assert_eq!(fx.events(), [settings_changed()]);
}

// Delete

#[tokio::test]
async fn delete_moves_the_folder_away_and_clears_selection() {
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    make(&fx, "B", "1.0");
    let level = write_world(&fx.paths().instance("9.9"));
    let (trasher, bin) = bin_trasher(&fx);
    fx.state.trasher = trasher;
    select(&fx, Some("9.9"));
    fx.clear_events();
    instance_delete_impl(&fx.state, "9.9".into()).await.unwrap();
    assert!(!fx.paths().instance("9.9").exists());
    assert_eq!(
        std::fs::read(bin.join("9.9/saves/world/level.dat")).unwrap(),
        b"world"
    );
    assert!(!level.exists());
    assert_eq!(selected(&fx), None);
    assert!(fx.paths().instance("1.0").join("instance.json").is_file());
    let events = fx.events();
    assert!(
        events.contains(&instances_changed()) && events.contains(&settings_changed()),
        "{events:?}"
    );
    // Deleting another instance keeps the selection.
    select(&fx, Some("1.0"));
    make(&fx, "C", "2.0");
    instance_delete_impl(&fx.state, "2.0".into()).await.unwrap();
    assert_eq!(selected(&fx).as_deref(), Some("1.0"));
}

#[tokio::test]
async fn delete_error_when_the_recycle_bin_refuses_leaves_everything_untouched() {
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    let level = write_world(&fx.paths().instance("9.9"));
    let before = std::fs::read(fx.paths().instance("9.9").join("instance.json")).unwrap();
    select(&fx, Some("9.9"));
    let calls = Arc::new(Mutex::new(0));
    fx.state.trasher = failing_trasher(calls.clone());
    fx.clear_events();
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::RecycleBinFailed);
    assert_eq!(dto.action, None);
    assert_eq!(dto.params.get("id").map(String::as_str), Some("9.9"));
    assert!(dto.detail.contains("the bin refused"), "{}", dto.detail);
    assert_eq!(*calls.lock().unwrap(), 1, "one attempt, no other route");
    assert_eq!(std::fs::read(&level).unwrap(), b"world");
    assert_eq!(
        std::fs::read(fx.paths().instance("9.9").join("instance.json")).unwrap(),
        before
    );
    assert_eq!(selected(&fx).as_deref(), Some("9.9"));
    assert!(fx.events().is_empty(), "{:?}", fx.events());
}

#[tokio::test]
async fn a_trasher_error_with_the_folder_gone_is_a_success_for_the_ui() {
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    write_world(&fx.paths().instance("9.9"));
    select(&fx, Some("9.9"));
    fx.state.trasher = Arc::new(|p| {
        std::fs::remove_dir_all(p)?;
        Err(Error::Io(std::io::Error::other("late failure")))
    });
    fx.clear_events();
    instance_delete_impl(&fx.state, "9.9".into()).await.unwrap();
    assert!(!fx.paths().instance("9.9").exists());
    assert_eq!(selected(&fx), None);
    assert!(fx.events().contains(&instances_changed()));
}

#[tokio::test]
async fn a_partial_move_is_its_own_code_and_refreshes_the_list() {
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    write_world(&fx.paths().instance("9.9"));
    select(&fx, Some("9.9"));
    fx.state.trasher = Arc::new(|p| {
        std::fs::remove_dir_all(p.join("saves"))?;
        Err(Error::Io(std::io::Error::other("bin full")))
    });
    fx.clear_events();
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::RecycleBinPartial);
    assert_eq!(dto.params.get("id").map(String::as_str), Some("9.9"));
    assert!(fx.paths().instance("9.9").join("options.txt").is_file());
    assert_eq!(
        selected(&fx).as_deref(),
        Some("9.9"),
        "the instance is still there"
    );
    assert_eq!(fx.events(), [instances_changed()]);
}

#[tokio::test]
async fn only_the_trashers_own_failure_is_a_recycle_bin_error() {
    // A core error that has nothing to do with the bin keeps its own code and the trasher is not
    // called: a too-new instance is refused by the core before any move.
    let mut fx = fixture().await;
    std::fs::create_dir_all(fx.paths().instance("newer")).unwrap();
    std::fs::write(
        fx.paths().instance("newer").join("instance.json"),
        r#"{"format":2}"#,
    )
    .unwrap();
    let calls = Arc::new(Mutex::new(0));
    fx.state.trasher = failing_trasher(calls.clone());
    let dto = instance_delete_impl(&fx.state, "newer".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::InstanceFormatTooNew);
    assert_eq!(*calls.lock().unwrap(), 0);
    assert!(fx.paths().instance("newer").join("instance.json").is_file());
}

#[tokio::test]
async fn delete_is_refused_for_the_active_instance() {
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    let level = write_world(&fx.paths().instance("9.9"));
    let (trasher, _bin) = bin_trasher(&fx);
    fx.state.trasher = trasher;
    *fx.state.active_instance.lock().unwrap() = Some("9.9".into());
    fx.clear_events();
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::InstanceRunning);
    assert!(level.is_file());
    assert!(fx.events().is_empty());
    // Another instance can be deleted while this one runs.
    make(&fx, "B", "1.0");
    instance_delete_impl(&fx.state, "1.0".into()).await.unwrap();
    *fx.state.active_instance.lock().unwrap() = None;
    instance_delete_impl(&fx.state, "9.9".into()).await.unwrap();
    assert!(!level.exists());
}

#[tokio::test]
async fn delete_refuses_unknown_and_unlisted_ids() {
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    let cli = fx.paths().instance("9.9-fabric");
    let level = write_world(&cli);
    let calls = Arc::new(Mutex::new(0));
    fx.state.trasher = failing_trasher(calls.clone());
    for (id, code) in [
        ("..\\x", ErrorCode::InstanceNotFound),
        ("../x", ErrorCode::InstanceNotFound),
        ("C:\\Windows", ErrorCode::InstanceNotFound),
        ("", ErrorCode::InstanceNotFound),
        ("fehlt", ErrorCode::InstanceNotFound),
        ("9.9-fabric", ErrorCode::InstanceNotFound),
    ] {
        let dto = instance_delete_impl(&fx.state, id.to_string())
            .await
            .unwrap_err();
        assert_eq!(dto.code, code, "{id:?}");
        assert_ne!(
            dto.code,
            ErrorCode::UnsafeData,
            "{id:?} must not say the data was rejected"
        );
    }
    assert_eq!(*calls.lock().unwrap(), 0, "the trasher was never reached");
    assert!(level.is_file(), "an unlisted folder must stay untouched");
    assert!(fx.events().is_empty());
}

#[tokio::test]
async fn deleting_an_unreadable_instance_is_allowed_and_a_too_new_one_is_not() {
    let mut fx = fixture().await;
    std::fs::create_dir_all(fx.paths().instance("bad")).unwrap();
    std::fs::write(fx.paths().instance("bad").join("instance.json"), "{").unwrap();
    let (trasher, bin) = bin_trasher(&fx);
    fx.state.trasher = trasher;
    instance_delete_impl(&fx.state, "bad".into()).await.unwrap();
    assert_eq!(std::fs::read(bin.join("bad/instance.json")).unwrap(), b"{");
}

#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
}

#[cfg(windows)]
#[tokio::test]
async fn a_linked_instance_folder_has_its_own_code_and_its_target_stays_untouched() {
    let mut fx = fixture().await;
    let outside = fx.dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("instance.json"), r#"{"format":1}"#).unwrap();
    let keep = outside.join("keep.txt");
    std::fs::write(&keep, b"keep").unwrap();
    std::fs::create_dir_all(fx.paths().root().join("instances")).unwrap();
    junction(&fx.paths().instance("linked"), &outside);
    let calls = Arc::new(Mutex::new(0));
    fx.state.trasher = failing_trasher(calls.clone());
    let dto = instance_delete_impl(&fx.state, "linked".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::InstanceLinked);
    assert_eq!(*calls.lock().unwrap(), 0);
    let dto = instance_select_impl(&fx.state, "linked".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::InstanceLinked);
    // The list does not show it and does not follow it.
    assert!(instances_list_impl(&fx.state)
        .await
        .unwrap()
        .instances
        .is_empty());
    assert_eq!(std::fs::read(&keep).unwrap(), b"keep");
    assert!(outside.join("instance.json").is_file());
}

#[test]
fn id_errors_map_only_the_grammar_failure_to_not_found() {
    let dto = instance_id_error(&Error::UnsafePath("..\\x".into()), "..\\x");
    assert_eq!(dto.code, ErrorCode::InstanceNotFound);
    assert_eq!(dto.action, None, "refreshing the list cannot help");
    // A folder name that names nothing keeps "refresh the list".
    let dto = instance_id_error(&Error::InstanceNotFound { id: "x".into() }, "x");
    assert_eq!(dto.action, Some(crate::dto::error::ErrorAction::Retry));
    let dto = instance_id_error(&Error::InstanceLinked { id: "a".into() }, "a");
    assert_eq!(dto.code, ErrorCode::InstanceLinked);
    // A path outside the id gate keeps the data-rejected code.
    assert_eq!(
        error_dto(&Error::UnsafePath("x".into())).code,
        ErrorCode::UnsafeData
    );
}

#[tokio::test]
async fn the_state_starts_with_no_active_instance() {
    // The default trasher is the real Recycle Bin one: it is never called by a normal test.
    let state = AppState::new(
        Paths::new(tempfile::tempdir().unwrap().path()),
        Endpoints::default(),
    );
    assert_eq!(*state.active_instance.lock().unwrap(), None);
}

#[tokio::test]
async fn a_failing_adoption_sends_no_event_so_a_reloading_ui_cannot_loop() {
    let fx = fixture().await;
    game_versions_impl(
        &fx.state,
        VersionFilter {
            release: true,
            snapshot: false,
            beta: false,
            alpha: false,
        },
    )
    .await
    .unwrap();
    // `instances` is a file: reading the folder fails for adopt and for list, nothing changes.
    std::fs::create_dir_all(fx.paths().root()).unwrap();
    std::fs::write(fx.paths().root().join("instances"), b"x").unwrap();
    fx.clear_events();
    assert!(instances_list_impl(&fx.state).await.is_err());
    assert!(fx.events().is_empty(), "{:?}", fx.events());
}

#[tokio::test]
async fn the_list_gives_up_waiting_for_a_hung_create_or_delete() {
    let fx = fixture().await;
    let _held = fx.state.instance_lock.lock().await;
    let started = std::time::Instant::now();
    let dto = instances_list_within(&fx.state, Duration::from_millis(150))
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::Busy);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn select_waits_for_the_lock_but_not_forever() {
    let fx = fixture().await;
    make(&fx, "A", "9.9");
    let _held = fx.state.instance_lock.lock().await;
    let dto = instance_select_within(&fx.state, "9.9".into(), Duration::from_millis(150))
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::Busy);
    assert_eq!(selected(&fx), None);
}

fn failing_with(failure: crate::recycle::RecycleFailure) -> crate::state::Trasher {
    Arc::new(move |_| -> CoreResult<()> { Err(Error::Io(std::io::Error::other(failure.clone()))) })
}

#[tokio::test]
async fn every_recycle_failure_reaches_the_ui_with_its_reason() {
    use crate::recycle::{BinReason, RecycleFailure};
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    for (failure, reason) in [
        (RecycleFailure::Refused(BinReason::TooLarge), "tooLarge"),
        (RecycleFailure::Refused(BinReason::BinOff), "binOff"),
        (RecycleFailure::Refused(BinReason::NoBin), "noBin"),
        (RecycleFailure::Refused(BinReason::Other), "other"),
        (RecycleFailure::Refused(BinReason::Unchecked), "unchecked"),
        (RecycleFailure::InUse("x".into()), "inUse"),
        (RecycleFailure::Declined("x".into()), "other"),
        (RecycleFailure::Failed("x".into()), "other"),
    ] {
        fx.state.trasher = failing_with(failure.clone());
        let dto = instance_delete_impl(&fx.state, "9.9".into())
            .await
            .unwrap_err();
        assert_eq!(dto.code, ErrorCode::RecycleBinFailed, "{failure:?}");
        assert_eq!(
            dto.params.get("reason").map(String::as_str),
            Some(reason),
            "{failure:?}"
        );
        assert!(fx.paths().instance("9.9").join("instance.json").is_file());
    }
    // A trasher that is not ours still gets the generic reason.
    fx.state.trasher = failing_trasher(Arc::default());
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.params.get("reason").map(String::as_str), Some("other"));
}

#[tokio::test]
async fn a_folder_gone_but_not_found_in_the_bin_is_never_reported_as_in_the_bin() {
    use crate::recycle::RecycleFailure;
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    select(&fx, Some("9.9"));
    fx.state.trasher = Arc::new(|p| {
        // What the shell did: the folder is gone and the bin does not have it.
        std::fs::remove_dir_all(p)?;
        Err(Error::Io(std::io::Error::other(
            RecycleFailure::Unconfirmed("no matching item in the Recycle Bin".into()),
        )))
    });
    fx.clear_events();
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::RecycleBinUnconfirmed);
    assert!(!fx.paths().instance("9.9").exists());
    // The instance is gone, so the list refreshes and the selection no longer names it.
    assert_eq!(selected(&fx), None);
    assert!(fx.events().contains(&instances_changed()));
}

#[tokio::test]
async fn the_detail_of_a_recycle_bin_error_carries_no_user_folder() {
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    fx.state.trasher = Arc::new(|_| -> CoreResult<()> {
        Err(Error::Io(std::io::Error::other(
            "windows error: C:\\Users\\someone\\AppData\\Local\\Temp\\home\\instances\\9.9: the destination is in use by another process (0x80270028)",
        )))
    });
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::RecycleBinFailed);
    assert!(
        !dto.detail.contains("someone") && !dto.detail.contains("Users"),
        "{}",
        dto.detail
    );
    assert!(
        dto.detail.contains("...\\9.9: the destination is in use"),
        "{}",
        dto.detail
    );
}

#[tokio::test]
async fn the_fixture_cannot_reach_the_real_recycle_bin() {
    let fx = fixture().await;
    let missing = fx.dir.path().join("nothing");
    let err = (fx.state.trasher)(&missing).unwrap_err();
    assert!(err.to_string().contains("no Recycle Bin"), "{err}");
}

#[tokio::test]
async fn a_typed_failure_with_the_folder_gone_is_never_reported_as_in_the_bin() {
    use crate::recycle::RecycleFailure;
    for failure in [
        RecycleFailure::Declined("x".into()),
        RecycleFailure::Failed("x".into()),
        RecycleFailure::InUse("x".into()),
    ] {
        let mut fx = fixture().await;
        make(&fx, "A", "9.9");
        select(&fx, Some("9.9"));
        let f = failure.clone();
        fx.state.trasher = Arc::new(move |p| {
            // The shell removed the folder and still reported a failure.
            std::fs::remove_dir_all(p)?;
            Err(Error::Io(std::io::Error::other(f.clone())))
        });
        fx.clear_events();
        let dto = instance_delete_impl(&fx.state, "9.9".into())
            .await
            .unwrap_err();
        assert_eq!(dto.code, ErrorCode::RecycleBinUnconfirmed, "{failure:?}");
        assert_eq!(selected(&fx), None, "{failure:?}");
        assert!(fx.events().contains(&instances_changed()), "{failure:?}");
    }
}

#[tokio::test]
async fn a_timed_out_or_stopped_call_is_unknown_and_a_pending_one_started_nothing() {
    use crate::recycle::RecycleFailure;
    let mut fx = fixture().await;
    make(&fx, "A", "9.9");
    for failure in [
        RecycleFailure::TimedOut,
        RecycleFailure::Aborted("x".into()),
    ] {
        fx.state.trasher = failing_with(failure.clone());
        fx.clear_events();
        let dto = instance_delete_impl(&fx.state, "9.9".into())
            .await
            .unwrap_err();
        assert_eq!(dto.code, ErrorCode::RecycleBinUnknown, "{failure:?}");
        assert!(fx.events().contains(&instances_changed()), "{failure:?}");
        assert!(
            !dto.params.contains_key("reason"),
            "no \"nothing was deleted\" claim"
        );
    }
    fx.state.trasher = failing_with(RecycleFailure::StillRunning);
    fx.clear_events();
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::RecycleBinPending);
    assert!(fx.events().is_empty());
    // A time-out after which the folder is gone is unknown as well, and the list refreshes.
    fx.state.trasher = Arc::new(|p| {
        std::fs::remove_dir_all(p)?;
        Err(Error::Io(std::io::Error::other(RecycleFailure::TimedOut)))
    });
    fx.clear_events();
    let dto = instance_delete_impl(&fx.state, "9.9".into())
        .await
        .unwrap_err();
    assert_eq!(dto.code, ErrorCode::RecycleBinUnknown);
    assert!(fx.events().contains(&instances_changed()));
}
