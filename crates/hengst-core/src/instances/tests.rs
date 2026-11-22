use super::*;
use crate::loaders::forge::LockTiming;
use crate::loaders::LoaderVersion;
use crate::Error;
use std::cell::RefCell;
use std::time::Duration;

const SHORT: LockTiming = LockTiming {
    notice_after: Duration::from_millis(50),
    limit: Duration::from_millis(200),
};

fn home() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    (dir, paths)
}

fn forge(version: &str) -> Option<InstanceLoader> {
    Some(InstanceLoader {
        kind: LoaderKind::Forge,
        version: Some(version.into()),
    })
}

fn new_instance(name: &str, game: &str, loader: Option<InstanceLoader>) -> NewInstance {
    NewInstance {
        name: name.into(),
        game: game.into(),
        loader,
    }
}

fn make(paths: &Paths, name: &str, game: &str) -> Instance {
    create(paths, &new_instance(name, game, None), 1).unwrap()
}

fn items(paths: &Paths) -> Vec<Listed> {
    let listing = list(paths).unwrap();
    assert!(!listing.incomplete);
    listing.items
}

fn write_meta(paths: &Paths, id: &str, json: &str) {
    let dir = paths.instance(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(INSTANCE_FILE), json).unwrap();
}

fn read_meta(paths: &Paths, id: &str) -> Vec<u8> {
    std::fs::read(paths.instance(id).join(INSTANCE_FILE)).unwrap()
}

/// A trasher that moves the folder into `bin` and records the calls.
struct Bin {
    dir: PathBuf,
    calls: RefCell<Vec<PathBuf>>,
}

impl Bin {
    fn new(dir: &Path) -> Self {
        let dir = dir.join("bin");
        std::fs::create_dir_all(&dir).unwrap();
        Self {
            dir,
            calls: RefCell::new(Vec::new()),
        }
    }
    fn trash(&self, p: &Path) -> Result<()> {
        self.calls.borrow_mut().push(p.to_path_buf());
        std::fs::rename(p, self.dir.join(p.file_name().unwrap()))?;
        Ok(())
    }
    fn called(&self) -> usize {
        self.calls.borrow().len()
    }
}

#[test]
fn default_id_matches_prepare() {
    assert_eq!(
        default_id("1.20.1", Some(LoaderKind::Forge)),
        "1.20.1-forge"
    );
    assert_eq!(default_id("1.21.11", None), "1.21.11");
    assert_eq!(
        default_id("1.8.9", Some(LoaderKind::LegacyFabric)),
        "1.8.9-legacy-fabric"
    );
}

#[test]
fn parses_default_ids_including_dashed_snapshots() {
    let known = |g: &str| {
        [
            "1.20.1",
            "1.8.9",
            "26.4-snapshot-6",
            "1.21.11",
            "1.14 Pre-Release 1",
        ]
        .contains(&g)
    };
    assert_eq!(
        parse_default_id("1.20.1-forge", known),
        Some(("1.20.1".into(), Some(LoaderKind::Forge)))
    );
    assert_eq!(
        parse_default_id("1.8.9-legacy-fabric", known),
        Some(("1.8.9".into(), Some(LoaderKind::LegacyFabric)))
    );
    assert_eq!(
        parse_default_id("26.4-snapshot-6-fabric", known),
        Some(("26.4-snapshot-6".into(), Some(LoaderKind::Fabric)))
    );
    assert_eq!(
        parse_default_id("1.21.11-neoforge", known),
        Some(("1.21.11".into(), Some(LoaderKind::NeoForge)))
    );
    assert_eq!(
        parse_default_id("1.14 Pre-Release 1", known),
        Some(("1.14 Pre-Release 1".into(), None))
    );
    assert_eq!(parse_default_id("my-pack", known), None);
    assert_eq!(parse_default_id("1.20.1-forge-2", known), None);
}

#[test]
fn create_writes_format_1_and_uses_the_cli_folder_name() {
    let (_d, paths) = home();
    let inst = create(
        &paths,
        &new_instance("1.20.1 Forge", "1.20.1", forge("47.4.28")),
        1234,
    )
    .unwrap();
    assert_eq!(inst.id, "1.20.1-forge");
    assert_eq!(inst.dir, paths.instance("1.20.1-forge"));
    let v: serde_json::Value = serde_json::from_slice(&read_meta(&paths, "1.20.1-forge")).unwrap();
    assert_eq!(v["format"], 1);
    assert_eq!(v["name"], "1.20.1 Forge");
    assert_eq!(v["game"], "1.20.1");
    assert_eq!(v["loader"]["kind"], "forge");
    assert_eq!(v["loader"]["version"], "47.4.28");
    assert_eq!(v["createdAt"], 1234);
    assert_eq!(load(&paths, "1.20.1-forge").unwrap(), inst);
}

#[test]
fn create_never_reuses_an_existing_folder() {
    let (_d, paths) = home();
    let level = paths.instance("1.20.1-forge").join("saves/world/level.dat");
    std::fs::create_dir_all(level.parent().unwrap()).unwrap();
    std::fs::write(&level, b"world").unwrap();
    let inst = create(
        &paths,
        &new_instance("1.20.1 Forge", "1.20.1", forge("47.4.28")),
        1,
    )
    .unwrap();
    assert_eq!(inst.id, "1.20.1-forge-2");
    assert_eq!(std::fs::read(&level).unwrap(), b"world");
    assert!(!paths.instance("1.20.1-forge").join(INSTANCE_FILE).exists());
}

#[test]
fn create_treats_folder_names_case_insensitively() {
    let (_d, paths) = home();
    std::fs::create_dir_all(paths.instance("1.20.1-FORGE")).unwrap();
    let inst = create(&paths, &new_instance("x", "1.20.1", forge("1")), 1).unwrap();
    assert_eq!(inst.id, "1.20.1-forge-2");
    // A case variant of the suffixed name is taken as well.
    std::fs::create_dir_all(paths.instance("1.21.11-2")).unwrap();
    std::fs::create_dir_all(paths.instance("1.21.11")).unwrap();
    assert_eq!(make(&paths, "n", "1.21.11").id, "1.21.11-3");
}

#[test]
fn create_gives_up_after_999_taken_folders() {
    let (_d, paths) = home();
    std::fs::create_dir_all(paths.instance("1.21.11")).unwrap();
    for n in 2..=999 {
        std::fs::create_dir_all(paths.instance(&format!("1.21.11-{n}"))).unwrap();
    }
    let err = create(&paths, &new_instance("n", "1.21.11", None), 1).unwrap_err();
    assert!(
        matches!(&err, Error::InstanceExists { id } if id == "1.21.11"),
        "{err:?}"
    );
}

#[test]
fn the_display_name_never_reaches_a_path() {
    let (d, paths) = home();
    let inst = make(&paths, "../../evil/CON:x", "1.21.11");
    assert_eq!(inst.id, "1.21.11");
    assert_eq!(inst.config.name, "../../evil/CON:x");
    let mut seen: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    seen.sort();
    assert_eq!(seen, ["instances", "tmp"].map(String::from));
    let inside: Vec<_> = std::fs::read_dir(paths.root().join("instances"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(inside, ["1.21.11"]);
}

#[test]
fn create_refuses_unsafe_or_reserved_game_ids() {
    let (_d, paths) = home();
    for bad in [
        "..",
        ".",
        "a/b",
        "a\\b",
        "C:",
        "CON",
        "nul",
        "com1",
        "LPT9.x",
        "1.8.",
        "1.8 ",
        " 1.8",
        "",
        "a\u{0}b",
        "\\\\srv\\share",
    ] {
        let r = create(&paths, &new_instance("n", bad, None), 1);
        assert!(matches!(r, Err(Error::UnsafePath(_))), "{bad:?}: {r:?}");
    }
    let long = "1".repeat(129);
    assert!(matches!(
        create(&paths, &new_instance("n", &long, None), 1),
        Err(Error::UnsafePath(_))
    ));
    assert!(!paths.root().join("instances").exists() || items(&paths).is_empty());
}

#[test]
fn latest_is_not_a_game_version_for_instances() {
    let (_d, paths) = home();
    for g in ["latest", "latest-release", "latest-snapshot"] {
        let r = create(&paths, &new_instance("n", g, None), 1);
        assert!(
            matches!(&r, Err(Error::UnknownVersion(v)) if v == g),
            "{r:?}"
        );
    }
}

#[test]
fn adopts_cli_folders_by_default_name() {
    let (_d, paths) = home();
    let level = paths.instance("1.20.1-forge").join("saves/w/level.dat");
    std::fs::create_dir_all(level.parent().unwrap()).unwrap();
    std::fs::write(&level, b"x").unwrap();
    std::fs::create_dir_all(paths.instance("random")).unwrap();
    std::fs::create_dir_all(paths.instance("1.21.11")).unwrap();
    let known = |g: &str| ["1.20.1", "1.21.11"].contains(&g);
    let mut adopted = adopt(&paths, known, 99).unwrap();
    adopted.sort();
    assert_eq!(adopted, ["1.20.1-forge", "1.21.11"]);
    let forge = load(&paths, "1.20.1-forge").unwrap();
    assert_eq!(forge.config.name, "1.20.1 Forge");
    assert_eq!(forge.config.game, "1.20.1");
    assert_eq!(forge.config.created_at, 99);
    assert_eq!(
        forge.config.loader,
        Some(InstanceLoader {
            kind: LoaderKind::Forge,
            version: None
        })
    );
    assert_eq!(load(&paths, "1.21.11").unwrap().config.name, "1.21.11");
    assert!(!paths.instance("random").join(INSTANCE_FILE).exists());
    assert_eq!(std::fs::read(&level).unwrap(), b"x");
    // A second run adopts nothing and keeps the files.
    assert!(adopt(&paths, known, 5).unwrap().is_empty());
    assert_eq!(load(&paths, "1.21.11").unwrap().config.created_at, 99);
}

#[test]
fn adopt_skips_links_and_ignores_missing_instances_folder() {
    let (_d, paths) = home();
    assert!(adopt(&paths, |_| true, 1).unwrap().is_empty());
    // A stray file and a name that is not an id grammar match are left alone.
    std::fs::create_dir_all(paths.root().join("instances")).unwrap();
    std::fs::write(paths.instance("1.21.11"), b"file").unwrap();
    std::fs::create_dir_all(paths.instance("\u{e4}")).unwrap();
    assert!(adopt(&paths, |_| true, 1).unwrap().is_empty());
}

#[test]
fn too_new_instance_is_listed_and_never_written() {
    let (_d, paths) = home();
    let json = r#"{"format": 2, "name": "Future", "game": "1.21.11", "createdAt": 1, "z": [1]}"#;
    write_meta(&paths, "future", json);
    let listed = items(&paths);
    assert_eq!(
        listed,
        [Listed::TooNew {
            id: "future".into(),
            format: 2
        }]
    );
    let err = update(&paths, "future", |c| c.name = "x".into()).unwrap_err();
    assert!(
        matches!(&err, Error::InstanceFormatTooNew { id, format: 2 } if id == "future"),
        "{err:?}"
    );
    assert!(matches!(
        load(&paths, "future"),
        Err(Error::InstanceFormatTooNew { format: 2, .. })
    ));
    assert!(matches!(
        record_launch(&paths, "future", 5, None),
        Err(Error::InstanceFormatTooNew { .. })
    ));
    assert_eq!(read_meta(&paths, "future"), json.as_bytes());
}

#[test]
fn broken_instance_is_listed_not_deleted() {
    let (_d, paths) = home();
    write_meta(&paths, "bad", "{");
    std::fs::create_dir_all(paths.instance("bad").join("saves")).unwrap();
    let listed = items(&paths);
    assert!(
        matches!(&listed[..], [Listed::Broken { id, .. }] if id == "bad"),
        "{listed:?}"
    );
    assert!(matches!(
        load(&paths, "bad"),
        Err(Error::InstanceUnreadable { .. })
    ));
    assert!(paths.instance("bad").join("saves").is_dir());
    assert_eq!(read_meta(&paths, "bad"), b"{");
}

#[test]
fn damaged_metadata_variants_are_all_broken_without_leaking_values() {
    let (_d, paths) = home();
    for (id, json) in [
        ("nofmt", r#"{"name":"x","game":"1.21.11","createdAt":1}"#),
        (
            "strfmt",
            r#"{"format":"1","name":"x","game":"1.21.11","createdAt":1}"#,
        ),
        (
            "zero",
            r#"{"format":0,"name":"x","game":"1.21.11","createdAt":1}"#,
        ),
        ("array", "[1]"),
        ("empty", ""),
        ("nogame", r#"{"format":1,"name":"x","createdAt":1}"#),
        (
            "secret",
            r#"{"format":1,"name":"FAKE-ACCESS-TOKEN-7f3a","game":5,"createdAt":1}"#,
        ),
    ] {
        write_meta(&paths, id, json);
    }
    let listed = items(&paths);
    assert_eq!(listed.len(), 7);
    for l in &listed {
        let Listed::Broken { reason, .. } = l else {
            panic!("{l:?}")
        };
        assert!(!reason.contains("FAKE-ACCESS-TOKEN"), "{reason}");
    }
}

#[test]
fn an_oversized_metadata_file_is_broken_and_not_read_whole() {
    let (_d, paths) = home();
    let dir = paths.instance("huge");
    std::fs::create_dir_all(&dir).unwrap();
    let f = std::fs::File::create(dir.join(INSTANCE_FILE)).unwrap();
    f.set_len(100 * 1024 * 1024).unwrap();
    let listed = items(&paths);
    assert!(
        matches!(&listed[..], [Listed::Broken { id, reason }] if id == "huge" && reason.contains("larger")),
        "{listed:?}"
    );
}

#[test]
fn list_survives_strays_and_odd_folders() {
    let (_d, paths) = home();
    assert!(items(&paths).is_empty(), "no instances folder");
    make(&paths, "Good", "1.21.11");
    let base = paths.root().join("instances");
    std::fs::write(base.join("stray.txt"), b"x").unwrap();
    std::fs::create_dir_all(base.join("no-metadata")).unwrap();
    std::fs::create_dir_all(base.join("\u{e4}\u{f6}")).unwrap();
    // instance.json as a directory is not a metadata file.
    std::fs::create_dir_all(base.join("dirfile").join(INSTANCE_FILE)).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let name = std::ffi::OsString::from_wide(&[0x61, 0xD800, 0x62]);
        // NTFS accepts a lone surrogate; if this volume does not, the case is moot.
        let _ = std::fs::create_dir_all(base.join(name));
    }
    let listed = items(&paths);
    assert!(
        matches!(&listed[..], [Listed::Ready(i)] if i.id == "1.21.11"),
        "{listed:?}"
    );
}

#[test]
fn list_handles_many_folders() {
    let (_d, paths) = home();
    for n in 0..1500 {
        write_meta(
            &paths,
            &format!("i{n}"),
            &format!(r#"{{"format":1,"name":"N{n}","game":"1.21.11","createdAt":1}}"#),
        );
    }
    assert_eq!(items(&paths).len(), 1500);
}

#[test]
fn list_sorts_by_last_played_then_name() {
    let (_d, paths) = home();
    for (id, name, played) in [
        ("a", "a", "null"),
        ("b", "b", "100"),
        ("c", "c", "200"),
        ("a2", "A2", "null"),
    ] {
        write_meta(
            &paths,
            id,
            &format!(
                r#"{{"format":1,"name":"{name}","game":"1.21.11","createdAt":1,"lastPlayed":{played}}}"#
            ),
        );
    }
    let ids: Vec<String> = items(&paths)
        .into_iter()
        .map(|l| match l {
            Listed::Ready(i) => i.id,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(ids, ["c", "b", "a", "a2"]);
}

#[test]
fn instance_dir_needs_a_valid_id_and_an_instance_json() {
    let (_d, paths) = home();
    std::fs::create_dir_all(paths.instance("1.20.1-forge").join("saves")).unwrap();
    create(&paths, &new_instance("Welt", "1.21.11", None), 1).unwrap();
    assert!(instance_dir(&paths, "1.21.11").is_ok());
    assert!(matches!(
        instance_dir(&paths, "1.20.1-forge"),
        Err(Error::InstanceNotFound { .. })
    ));
    assert!(matches!(
        instance_dir(&paths, "fehlt"),
        Err(Error::InstanceNotFound { .. })
    ));
    for bad in ["..", "..\\x", "a/b", ""] {
        assert!(
            matches!(instance_dir(&paths, bad), Err(Error::UnsafePath(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn instance_dir_refuses_every_traversal_shape() {
    let (d, paths) = home();
    make(&paths, "Welt", "1.21.11");
    // A real file outside instances/ that a traversal would reach.
    std::fs::write(d.path().join(INSTANCE_FILE), b"{}").unwrap();
    let abs = d.path().to_string_lossy().into_owned();
    let unc = format!("\\\\localhost\\{}", abs.replace(':', "$"));
    for bad in [
        "..",
        ".",
        "../instances/1.21.11",
        "..\\instances\\1.21.11",
        "1.21.11/..",
        "1.21.11\\",
        "/1.21.11",
        "\\1.21.11",
        "C:",
        "C:\\Windows",
        "C:1.21.11",
        abs.as_str(),
        unc.as_str(),
        "\\\\?\\C:\\x",
        "1.21.11:stream",
        "a\u{0}b",
        "1.21.11\u{0}",
        "name.",
        "name ",
        " name",
        "CON",
        "con",
        "Nul.txt",
        "AUX",
        "PRN",
        "COM1",
        "lpt9",
        "\u{e4}",
        "1.21.11*",
        "1.21.11?",
        "a|b",
        "a<b",
        "~1",
        "",
    ] {
        let r = instance_dir(&paths, bad);
        assert!(matches!(r, Err(Error::UnsafePath(_))), "{bad:?}: {r:?}");
        assert!(
            matches!(load(&paths, bad), Err(Error::UnsafePath(_))),
            "{bad:?}"
        );
    }
    let long = "a".repeat(129);
    assert!(matches!(
        instance_dir(&paths, &long),
        Err(Error::UnsafePath(_))
    ));
}

#[test]
fn instance_dir_wants_the_exact_folder_name() {
    let (_d, paths) = home();
    make(&paths, "Welt", "1.20.1");
    // The file system may resolve `1.20.1-X` to the folder `1.20.1`; the id must still be exact.
    std::fs::create_dir_all(paths.instance("Pack")).unwrap();
    std::fs::write(paths.instance("Pack").join(INSTANCE_FILE), b"{}").unwrap();
    assert!(instance_dir(&paths, "Pack").is_ok());
    assert!(matches!(
        instance_dir(&paths, "pack"),
        Err(Error::InstanceNotFound { .. })
    ));
    assert!(matches!(
        instance_dir(&paths, "PACK"),
        Err(Error::InstanceNotFound { .. })
    ));
}

#[test]
fn unknown_fields_survive_an_update() {
    let (_d, paths) = home();
    write_meta(
        &paths,
        "keep",
        r#"{"format":1,"name":"A","game":"1.21.11","createdAt":1,"notes":"x","nested":{"k":[1,2]}}"#,
    );
    let inst = load(&paths, "keep").unwrap();
    assert_eq!(inst.config.extra["notes"], "x");
    update(&paths, "keep", |c| c.memory_mb = Some(4096)).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&read_meta(&paths, "keep")).unwrap();
    assert_eq!(v["notes"], "x");
    assert_eq!(v["nested"]["k"][1], 2);
    assert_eq!(v["memoryMb"], 4096);
}

const NESTED: &str = r#"{
  "format": 1, "name": "A", "game": "1.21.11", "createdAt": 1,
  "loader": {"kind": "fabric", "version": null,
             "channel": "beta", "meta": {"a": [1, {"b": 2}], "c": {"d": null}}},
  "top": {"deep": {"deeper": [true, "x"]}}
}"#;

fn nested_unknowns_intact(paths: &Paths, id: &str) {
    let v: serde_json::Value = serde_json::from_slice(&read_meta(paths, id)).unwrap();
    assert_eq!(v["loader"]["channel"], "beta");
    assert_eq!(
        v["loader"]["meta"],
        serde_json::json!({"a": [1, {"b": 2}], "c": {"d": null}})
    );
    assert_eq!(
        v["top"],
        serde_json::json!({"deep": {"deeper": [true, "x"]}})
    );
}

#[test]
fn unknown_fields_nested_under_loader_survive_update_and_record_launch() {
    let (_d, paths) = home();
    write_meta(&paths, "1.21.11-fabric", NESTED);
    update(&paths, "1.21.11-fabric", |c| c.memory_mb = Some(1024)).unwrap();
    nested_unknowns_intact(&paths, "1.21.11-fabric");
    record_launch(&paths, "1.21.11-fabric", 77, Some(&resolved("0.17.3"))).unwrap();
    nested_unknowns_intact(&paths, "1.21.11-fabric");
    let v: serde_json::Value =
        serde_json::from_slice(&read_meta(&paths, "1.21.11-fabric")).unwrap();
    assert_eq!(v["loader"]["version"], "0.17.3");
    assert_eq!(v["loader"]["kind"], "fabric");
    assert_eq!(v["lastPlayed"], 77);
    // The loader can be changed by the caller; a null replaces the object.
    update(&paths, "1.21.11-fabric", |c| c.loader = None).unwrap();
    let v: serde_json::Value =
        serde_json::from_slice(&read_meta(&paths, "1.21.11-fabric")).unwrap();
    assert!(v["loader"].is_null());
    assert_eq!(v["top"]["deep"]["deeper"][1], "x");
}

#[test]
fn adoption_leaves_an_existing_file_with_its_nested_fields_alone() {
    let (_d, paths) = home();
    write_meta(&paths, "1.21.11-fabric", NESTED);
    std::fs::create_dir_all(paths.instance("1.20.1")).unwrap();
    let before = read_meta(&paths, "1.21.11-fabric");
    let adopted = adopt(&paths, |g| g == "1.21.11" || g == "1.20.1", 5).unwrap();
    assert_eq!(adopted, ["1.20.1"]);
    assert_eq!(read_meta(&paths, "1.21.11-fabric"), before);
}

#[test]
fn update_works_on_the_file_as_it_is_now_not_on_a_stale_copy() {
    let (_d, paths) = home();
    let stale = make(&paths, "A", "1.21.11");
    record_launch(&paths, "1.21.11", 500, None).unwrap();
    // A rename built from the old listing must not roll back lastPlayed.
    update(&paths, &stale.id, |c| c.name = "B".into()).unwrap();
    let now = load(&paths, "1.21.11").unwrap();
    assert_eq!(
        (now.config.name.as_str(), now.config.last_played),
        ("B", Some(500))
    );
    // Fields another writer added are kept too.
    let mut v: serde_json::Value = serde_json::from_slice(&read_meta(&paths, "1.21.11")).unwrap();
    v["added"] = "later".into();
    std::fs::write(
        paths.instance("1.21.11").join(INSTANCE_FILE),
        serde_json::to_vec(&v).unwrap(),
    )
    .unwrap();
    update(&paths, "1.21.11", |c| c.name = "C".into()).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&read_meta(&paths, "1.21.11")).unwrap();
    assert_eq!(
        (v["added"].as_str(), v["name"].as_str()),
        (Some("later"), Some("C"))
    );
}

#[test]
fn update_validates_the_name_and_the_id_before_touching_anything() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    update(&paths, "1.21.11", |c| c.name = "  Neu  ".into()).unwrap();
    assert_eq!(load(&paths, "1.21.11").unwrap().config.name, "Neu");
    let before = read_meta(&paths, "1.21.11");
    let r = update(&paths, "1.21.11", |c| c.name = "   ".into());
    assert!(matches!(r, Err(Error::InvalidInstanceName(_))));
    assert_eq!(read_meta(&paths, "1.21.11"), before);
    assert!(matches!(
        update(&paths, "..", |_| {}),
        Err(Error::UnsafePath(_))
    ));
}

#[test]
fn update_never_recreates_a_deleted_instance_or_overwrites_damage() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    write_meta(&paths, "1.21.11", "{ broken");
    assert!(matches!(
        update(&paths, "1.21.11", |_| {}),
        Err(Error::InstanceUnreadable { .. })
    ));
    assert_eq!(read_meta(&paths, "1.21.11"), b"{ broken");
    std::fs::remove_dir_all(paths.instance("1.21.11")).unwrap();
    assert!(matches!(
        update(&paths, "1.21.11", |_| {}),
        Err(Error::InstanceNotFound { .. })
    ));
    assert!(
        !paths.instance("1.21.11").exists(),
        "update must not create the folder"
    );
}

#[test]
fn ids_that_name_no_instance_leave_no_lock_file() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    let locks = |paths: &Paths| -> Vec<String> {
        std::fs::read_dir(paths.tmp())
            .map(|rd| {
                rd.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .filter(|n| n.starts_with("instance-"))
                    .collect()
            })
            .unwrap_or_default()
    };
    let known = locks(&paths);
    std::fs::create_dir_all(paths.instance("cli-only")).unwrap();
    let bin = Bin::new(paths.root());
    for id in [
        "fehlt",
        "cli-only",
        "..",
        "a/b",
        "CON",
        "x".repeat(200).as_str(),
    ] {
        let _ = update(&paths, id, |_| {});
        let _ = record_launch(&paths, id, 1, None);
        let _ = delete(&paths, id, &|p| bin.trash(p));
    }
    assert_eq!(locks(&paths), known);
}

#[test]
fn names_are_validated() {
    for bad in ["", "   ", "a\nb", "a\u{0}b", "tab\there", "\u{7f}"] {
        assert!(
            matches!(validate_name(bad), Err(Error::InvalidInstanceName(_))),
            "{bad:?}"
        );
    }
    assert!(matches!(
        validate_name(&"a".repeat(65)),
        Err(Error::InvalidInstanceName(_))
    ));
    assert_eq!(validate_name(&"a".repeat(64)).unwrap().len(), 64);
    assert_eq!(
        validate_name(&"\u{dc}".repeat(64)).unwrap().chars().count(),
        64
    );
    assert_eq!(validate_name("  Welt  ").unwrap(), "Welt");
    assert_eq!(
        validate_name("\u{dc}berleben 1.21").unwrap(),
        "\u{dc}berleben 1.21"
    );
    assert_eq!(validate_name("a  b").unwrap(), "a  b");
    // 64 characters after trimming, not before.
    let padded = format!(" {} ", "a".repeat(64));
    assert_eq!(validate_name(&padded).unwrap().len(), 64);
}

#[test]
fn create_refuses_an_invalid_name_without_touching_the_disk() {
    let (_d, paths) = home();
    let r = create(&paths, &new_instance("  ", "1.21.11", None), 1);
    assert!(matches!(r, Err(Error::InvalidInstanceName(_))));
    assert!(!paths.instance("1.21.11").exists());
}

#[test]
fn launch_options_use_instance_values_before_defaults() {
    let (_d, paths) = home();
    let mut inst = create(&paths, &new_instance("n", "1.20.1", forge("47.4.28")), 1).unwrap();
    let o = launch_options(
        &inst,
        Account::offline("Dev"),
        2048,
        Some("C:/java.exe".into()),
    );
    assert_eq!(o.version, "1.20.1");
    assert_eq!(o.instance.as_deref(), Some("1.20.1-forge"));
    assert_eq!(o.memory_mb, 2048);
    assert_eq!(o.java, Some(PathBuf::from("C:/java.exe")));
    let l = o.loader.unwrap();
    assert_eq!(
        (l.kind, l.version.as_deref()),
        (LoaderKind::Forge, Some("47.4.28"))
    );
    inst.config.memory_mb = Some(4096);
    inst.config.java = Some("D:/j/java.exe".into());
    let o = launch_options(&inst, Account::offline("Dev"), 2048, None);
    assert_eq!(o.memory_mb, 4096);
    assert_eq!(o.java, Some(PathBuf::from("D:/j/java.exe")));
    inst.config.loader = None;
    assert!(launch_options(&inst, Account::offline("Dev"), 2048, None)
        .loader
        .is_none());
}

fn resolved(version: &str) -> ResolvedLoader {
    ResolvedLoader {
        kind: LoaderKind::Fabric,
        version: LoaderVersion {
            id: version.into(),
            maven: version.into(),
            stable: true,
        },
    }
}

#[test]
fn record_launch_pins_the_resolved_loader_once() {
    let (_d, paths) = home();
    let known = |g: &str| g == "1.21.11";
    std::fs::create_dir_all(paths.instance("1.21.11-fabric")).unwrap();
    adopt(&paths, known, 1).unwrap();
    record_launch(&paths, "1.21.11-fabric", 50, Some(&resolved("0.17.3"))).unwrap();
    let i = load(&paths, "1.21.11-fabric").unwrap();
    assert_eq!(i.config.last_played, Some(50));
    assert_eq!(i.config.loader.unwrap().version.as_deref(), Some("0.17.3"));
    record_launch(&paths, "1.21.11-fabric", 60, Some(&resolved("0.18.0"))).unwrap();
    let i = load(&paths, "1.21.11-fabric").unwrap();
    assert_eq!(i.config.last_played, Some(60));
    assert_eq!(i.config.loader.unwrap().version.as_deref(), Some("0.17.3"));
    // Without a loader the call only sets the time.
    make(&paths, "v", "1.20.1");
    record_launch(&paths, "1.20.1", 7, Some(&resolved("9"))).unwrap();
    let i = load(&paths, "1.20.1").unwrap();
    assert_eq!((i.config.last_played, i.config.loader), (Some(7), None));
}

#[test]
fn loader_kind_serializes_as_slug() {
    for kind in [
        LoaderKind::Fabric,
        LoaderKind::Quilt,
        LoaderKind::LegacyFabric,
        LoaderKind::NeoForge,
        LoaderKind::Forge,
    ] {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{}\"", kind.slug()));
        assert_eq!(serde_json::from_str::<LoaderKind>(&json).unwrap(), kind);
    }
    assert_eq!(
        serde_json::to_string(&LoaderKind::NeoForge).unwrap(),
        "\"neoforge\""
    );
    assert_eq!(
        serde_json::to_string(&LoaderKind::LegacyFabric).unwrap(),
        "\"legacy-fabric\""
    );
    assert!(serde_json::from_str::<LoaderKind>("\"NeoForge\"").is_err());
}

// Deletion

#[test]
fn delete_moves_only_the_instance_folder_through_the_trasher() {
    let (d, paths) = home();
    make(&paths, "A", "1.21.11");
    make(&paths, "B", "1.20.1");
    let jar = paths.client_jar("1.21.11");
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    std::fs::write(&jar, b"jar").unwrap();
    std::fs::create_dir_all(paths.libraries()).unwrap();
    let bin = Bin::new(d.path());
    delete(&paths, "1.21.11", &|p| bin.trash(p)).unwrap();
    assert_eq!(bin.calls.borrow()[..], [paths.instance("1.21.11")]);
    assert!(!paths.instance("1.21.11").exists());
    assert!(paths.instance("1.20.1").join(INSTANCE_FILE).is_file());
    assert!(jar.is_file() && paths.libraries().is_dir());
}

#[test]
fn a_failed_trash_leaves_everything_in_place_and_never_deletes_for_good() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    let level = paths.instance("1.21.11").join("saves/w/level.dat");
    std::fs::create_dir_all(level.parent().unwrap()).unwrap();
    std::fs::write(&level, b"world").unwrap();
    let before = read_meta(&paths, "1.21.11");
    let calls = RefCell::new(0);
    let err = delete(&paths, "1.21.11", &|_| {
        *calls.borrow_mut() += 1;
        Err(Error::Io(std::io::Error::other("bin refused")))
    })
    .unwrap_err();
    assert!(matches!(err, Error::Io(_)), "{err:?}");
    assert_eq!(*calls.borrow(), 1, "no second attempt, no other route");
    assert_eq!(std::fs::read(&level).unwrap(), b"world");
    assert_eq!(read_meta(&paths, "1.21.11"), before);
    assert_eq!(items(&paths).len(), 1);
}

#[test]
fn a_trasher_that_reports_success_but_leaves_the_folder_is_an_error() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    let r = delete(&paths, "1.21.11", &|_| Ok(()));
    assert!(matches!(r, Err(Error::Io(_))), "{r:?}");
    assert!(paths.instance("1.21.11").is_dir());
}

#[test]
fn delete_refuses_unknown_unsafe_unlisted_and_too_new_ids() {
    let (d, paths) = home();
    make(&paths, "A", "9.9");
    let level = paths.instance("9.9-fabric").join("saves/world/level.dat");
    std::fs::create_dir_all(level.parent().unwrap()).unwrap();
    std::fs::write(&level, b"w").unwrap();
    write_meta(&paths, "future", r#"{"format":2}"#);
    let bin = Bin::new(d.path());
    for bad in ["..\\x", "..", "", "CON", "C:\\x", "a/b", "name."] {
        assert!(
            matches!(
                delete(&paths, bad, &|p| bin.trash(p)),
                Err(Error::UnsafePath(_))
            ),
            "{bad:?}"
        );
    }
    assert!(matches!(
        delete(&paths, "fehlt", &|p| bin.trash(p)),
        Err(Error::InstanceNotFound { .. })
    ));
    assert!(matches!(
        delete(&paths, "9.9-fabric", &|p| bin.trash(p)),
        Err(Error::InstanceNotFound { .. })
    ));
    assert!(matches!(
        delete(&paths, "future", &|p| bin.trash(p)),
        Err(Error::InstanceFormatTooNew { format: 2, .. })
    ));
    assert_eq!(bin.called(), 0);
    assert!(level.is_file());
    assert_eq!(read_meta(&paths, "future"), br#"{"format":2}"#);
    delete(&paths, "9.9", &|p| bin.trash(p)).unwrap();
}

// Links and junctions

#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    let out = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .expect("cmd runs");
    assert!(
        out.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[cfg(windows)]
#[test]
fn a_junction_out_of_instances_is_skipped_refused_and_never_followed() {
    let (d, paths) = home();
    make(&paths, "Real", "1.21.11");
    let outside = d.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let meta = r#"{"format":1,"name":"Outside","game":"1.21.11","createdAt":1}"#;
    std::fs::write(outside.join(INSTANCE_FILE), meta).unwrap();
    std::fs::write(outside.join("keep.txt"), b"keep").unwrap();
    let link = paths.instance("evil");
    junction(&link, &outside);

    let listed = items(&paths);
    assert!(
        matches!(&listed[..], [Listed::Ready(i)] if i.id == "1.21.11"),
        "{listed:?}"
    );
    assert!(matches!(
        instance_dir(&paths, "evil"),
        Err(Error::InstanceLinked { .. })
    ));
    assert!(matches!(
        load(&paths, "evil"),
        Err(Error::InstanceLinked { .. })
    ));
    let bin = Bin::new(d.path());
    assert!(matches!(
        delete(&paths, "evil", &|p| bin.trash(p)),
        Err(Error::InstanceLinked { .. })
    ));
    assert_eq!(bin.called(), 0);
    assert!(adopt(&paths, |_| true, 1).unwrap().is_empty());
    assert_eq!(std::fs::read(outside.join("keep.txt")).unwrap(), b"keep");
    assert_eq!(
        std::fs::read(outside.join(INSTANCE_FILE)).unwrap(),
        meta.as_bytes()
    );
    std::fs::remove_dir(&link).unwrap();
}

#[cfg(windows)]
#[test]
fn a_junction_without_metadata_is_not_adopted() {
    let (d, paths) = home();
    let outside = d.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(paths.root().join("instances")).unwrap();
    let link = paths.instance("1.21.11");
    junction(&link, &outside);
    assert!(adopt(&paths, |g| g == "1.21.11", 1).unwrap().is_empty());
    assert!(!outside.join(INSTANCE_FILE).exists());
    std::fs::remove_dir(&link).unwrap();
}

#[cfg(windows)]
#[test]
fn a_symlinked_metadata_file_is_not_followed() {
    let (d, paths) = home();
    let secret = d.path().join("secret.json");
    std::fs::write(
        &secret,
        r#"{"format":1,"name":"S","game":"1.21.11","createdAt":1}"#,
    )
    .unwrap();
    std::fs::create_dir_all(paths.instance("x")).unwrap();
    // Creating a file symlink needs a privilege; without it the case cannot be built here.
    if std::os::windows::fs::symlink_file(&secret, paths.instance("x").join(INSTANCE_FILE)).is_ok()
    {
        assert!(items(&paths).is_empty());
        assert!(matches!(
            instance_dir(&paths, "x"),
            Err(Error::InstanceNotFound { .. })
        ));
    }
}

// Locking

#[test]
fn a_held_instance_lock_makes_writers_time_out_with_the_core_lock_error() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    let held = lock_instance(&paths, "1.21.11", SHORT).unwrap();
    let err = lock_instance(&paths, "1.21.11", SHORT).unwrap_err();
    assert!(matches!(err, Error::LockTimeout { .. }), "{err:?}");
    // The lock file is outside the instance folder, so the folder can still be moved away.
    assert!(!paths.instance("1.21.11").join("instance.lock").exists());
    drop(held);
    update(&paths, "1.21.11", |_| {}).unwrap();
}

#[test]
fn deleting_an_unreadable_instance_moves_the_folder_unchanged() {
    let (d, paths) = home();
    write_meta(&paths, "bad", "{ not json");
    let level = paths.instance("bad").join("saves/w/level.dat");
    std::fs::create_dir_all(level.parent().unwrap()).unwrap();
    std::fs::write(&level, b"world").unwrap();
    let bin = Bin::new(d.path());
    delete(&paths, "bad", &|p| bin.trash(p)).unwrap();
    assert_eq!(bin.called(), 1);
    assert!(!paths.instance("bad").exists());
    let moved = bin.dir.join("bad");
    assert_eq!(
        std::fs::read(moved.join(INSTANCE_FILE)).unwrap(),
        b"{ not json"
    );
    assert_eq!(
        std::fs::read(moved.join("saves/w/level.dat")).unwrap(),
        b"world"
    );
}

fn instance_with_world(paths: &Paths) -> PathBuf {
    make(paths, "A", "1.21.11");
    let dir = paths.instance("1.21.11");
    std::fs::create_dir_all(dir.join("saves/w")).unwrap();
    std::fs::write(dir.join("saves/w/level.dat"), b"w").unwrap();
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("options.txt"), b"o").unwrap();
    dir
}

#[test]
fn a_trasher_that_removes_part_and_then_fails_gives_the_partly_moved_error() {
    let (_d, paths) = home();
    let dir = instance_with_world(&paths);
    let r = delete(&paths, "1.21.11", &|p| {
        std::fs::remove_dir_all(p.join("saves"))?;
        Err(Error::Io(std::io::Error::other("bin full")))
    });
    assert!(
        matches!(&r, Err(Error::InstancePartlyMoved { id }) if id == "1.21.11"),
        "{r:?}"
    );
    assert!(dir.join("options.txt").is_file(), "the rest stays in place");
}

#[test]
fn a_trasher_that_moves_everything_and_then_fails_is_a_success() {
    // The instance is gone from its folder (in the bin or deleted by the shell): the caller has to
    // refresh its list, not hear that "the rest is still in place".
    let (_d, paths) = home();
    instance_with_world(&paths);
    let r = delete(&paths, "1.21.11", &|p| {
        std::fs::remove_dir_all(p)?;
        Err(Error::Io(std::io::Error::other("late failure")))
    });
    assert!(r.is_ok(), "{r:?}");
    assert!(!paths.instance("1.21.11").exists());
    assert!(items(&paths).is_empty());
}

#[test]
fn a_trasher_error_with_the_folder_gone_is_ok_for_a_broken_instance_too() {
    let (_d, paths) = home();
    write_meta(&paths, "bad", "{ not json");
    let r = delete(&paths, "bad", &|p| {
        std::fs::remove_dir_all(p)?;
        Err(Error::Io(std::io::Error::other("late failure")))
    });
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn a_failure_that_changed_nothing_still_returns_the_original_error() {
    let (_d, paths) = home();
    instance_with_world(&paths);
    // Adding entries is not "missing" entries.
    let r = delete(&paths, "1.21.11", &|p| {
        std::fs::write(p.join("new.txt"), b"x")?;
        Err(Error::Io(std::io::Error::other("refused")))
    });
    assert!(matches!(r, Err(Error::Io(_))), "{r:?}");
}

#[test]
fn partial_detection_is_shallow_and_says_so() {
    // Documented limit: a failure that removed files below the top level is not noticed.
    let (_d, paths) = home();
    instance_with_world(&paths);
    let r = delete(&paths, "1.21.11", &|p| {
        std::fs::remove_file(p.join("saves/w/level.dat"))?;
        Err(Error::Io(std::io::Error::other("refused")))
    });
    assert!(matches!(r, Err(Error::Io(_))), "{r:?}");
}

// Listing

#[test]
fn a_scan_with_iterator_errors_is_marked_incomplete() {
    let (d, _paths) = home();
    std::fs::create_dir_all(d.path().join("a")).unwrap();
    std::fs::create_dir_all(d.path().join("b")).unwrap();
    let entry = |n: &str| Ok((std::ffi::OsString::from(n), d.path().join(n)));
    let bad = || Err(std::io::Error::other("iterator failed"));
    let (ids, incomplete) = scan(vec![entry("a"), bad(), entry("b")].into_iter());
    assert_eq!(ids, ["a", "b"]);
    assert!(incomplete);
    let (ids, incomplete) = scan(vec![entry("a"), entry("b")].into_iter());
    assert_eq!((ids.len(), incomplete), (2, false));
    // An iterator that fails forever ends the scan instead of spinning.
    let (ids, incomplete) = scan(std::iter::repeat_with(bad));
    assert!(ids.is_empty() && incomplete);
}

#[test]
fn a_clean_listing_is_not_marked_incomplete() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    let listing = list(&paths).unwrap();
    assert_eq!((listing.items.len(), listing.incomplete), (1, false));
    assert_eq!(
        list(&Paths::new(paths.root().join("nope"))).unwrap().items,
        []
    );
}

// Adoption

#[test]
fn adopt_only_treats_a_missing_file_as_no_instance_json() {
    let (_d, paths) = home();
    // instance.json is there, as a directory: not "missing", so nothing is written over it.
    std::fs::create_dir_all(paths.instance("1.21.11").join(INSTANCE_FILE)).unwrap();
    assert!(adopt(&paths, |_| true, 1).unwrap().is_empty());
    assert!(paths.instance("1.21.11").join(INSTANCE_FILE).is_dir());
}

#[test]
fn adopt_never_replaces_a_file_that_appears_before_the_write() {
    let (_d, paths) = home();
    std::fs::create_dir_all(paths.instance("1.21.11")).unwrap();
    let adopted = adopt_with(&paths, |g| g == "1.21.11", 1, &|dir| {
        std::fs::write(dir.join(INSTANCE_FILE), b"mine").unwrap();
    })
    .unwrap();
    assert!(adopted.is_empty());
    assert_eq!(read_meta(&paths, "1.21.11"), b"mine");
    let left: Vec<_> = std::fs::read_dir(paths.instance("1.21.11"))
        .unwrap()
        .collect();
    assert_eq!(left.len(), 1, "no temp file is left behind");
}

// Errors

#[test]
fn link_and_partial_errors_are_distinct_variants() {
    assert_eq!(
        Error::InstanceLinked { id: "x".into() }.to_string(),
        "instance folder x is a link, which the launcher does not follow"
    );
    assert_eq!(
        Error::InstancePartlyMoved { id: "x".into() }.to_string(),
        "instance x was only partly moved to the Recycle Bin"
    );
}

// Carried from the reviews of the instances module

#[test]
fn unpinning_the_loader_version_writes_null_and_the_reloaded_instance_has_none() {
    let (_d, paths) = home();
    create(&paths, &new_instance("F", "1.20.1", forge("47.2.0")), 1).unwrap();
    let id = "1.20.1-forge";
    update(&paths, id, |c| c.loader.as_mut().unwrap().version = None).unwrap();
    let on_disk: serde_json::Value = serde_json::from_slice(&read_meta(&paths, id)).unwrap();
    assert_eq!(on_disk["loader"]["version"], serde_json::Value::Null);
    assert_eq!(on_disk["loader"]["kind"], "forge");
    let loaded = load(&paths, id).unwrap();
    assert_eq!(loaded.config.loader.unwrap().version, None);
}

#[test]
fn a_panic_in_the_update_closure_writes_nothing_and_releases_the_lock() {
    let (_d, paths) = home();
    make(&paths, "A", "1.21.11");
    let before = read_meta(&paths, "1.21.11");
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = update(&paths, "1.21.11", |c| {
            c.name = "changed before the panic".into();
            panic!("closure failed");
        });
    }));
    assert!(caught.is_err());
    assert_eq!(read_meta(&paths, "1.21.11"), before);
    // The lock is free again: a writer does not wait for the timeout.
    let started = std::time::Instant::now();
    update(&paths, "1.21.11", |c| c.name = "after".into()).unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(load(&paths, "1.21.11").unwrap().config.name, "after");
}

#[cfg(windows)]
#[test]
fn a_name_the_system_cannot_look_up_marks_the_scan_incomplete_for_directories_and_files() {
    // `|` is not valid in a Windows name: the lookup fails with an error other than "not there".
    let (d, _paths) = home();
    let bad = d.path().join("a|b");
    assert!(plain_dir(&bad).is_err(), "the directory arm");
    assert!(plain_file(&bad).is_err(), "the file arm");
    assert!(!is_plain_dir(&bad) && !is_plain_file(&bad));
    let entries = vec![Ok((std::ffi::OsString::from("x"), bad.clone()))];
    let (ids, incomplete) = scan(entries.into_iter());
    assert!(ids.is_empty() && incomplete);
    // Plain absence is not an error.
    assert_eq!(plain_dir(&d.path().join("nothing")), Ok(false));
    assert_eq!(plain_file(&d.path().join("nothing")), Ok(false));
}

#[test]
fn the_exact_name_scan_reports_iterator_errors_instead_of_not_found() {
    let name = |n: &str| Ok(std::ffi::OsString::from(n));
    let bad = || Err(std::io::Error::other("iterator failed"));
    assert!(exact_entry(vec![name("a"), bad(), name("b")].into_iter(), "c").is_err());
    assert!(exact_entry(vec![name("a"), bad(), name("b")].into_iter(), "b").unwrap());
    assert!(!exact_entry(vec![name("a"), name("b")].into_iter(), "c").unwrap());
    // An iterator that fails forever ends with the error.
    assert!(exact_entry(std::iter::repeat_with(bad), "c").is_err());
}

fn stale_part(dir: &Path, base: &str) -> PathBuf {
    // Another process's temp file, untouched for longer than the cleanup age.
    let part = dir.join(format!("{base}.4000000.1.part"));
    let f = std::fs::File::create(&part).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    f.set_modified(old).unwrap();
    part
}

#[test]
fn adopt_removes_a_stale_temp_file_next_to_the_instance_json() {
    let (_d, paths) = home();
    let dir = paths.instance("1.21.11");
    std::fs::create_dir_all(&dir).unwrap();
    let part = stale_part(&dir, INSTANCE_FILE);
    assert_eq!(adopt(&paths, |g| g == "1.21.11", 1).unwrap(), ["1.21.11"]);
    assert!(!part.exists());
    assert!(dir.join(INSTANCE_FILE).is_file());
}
