use std::collections::BTreeMap;
use std::path::Path;
use ts_rs::Config;

const BINDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc/bindings");

/// Line endings do not count: a checkout with `core.autocrlf` rewrites them.
fn read_dir(dir: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .map(|e| {
            let e = e.unwrap();
            let text = std::fs::read_to_string(e.path()).unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                text.replace("\r\n", "\n"),
            )
        })
        .collect()
}

#[test]
fn bindings_are_current() {
    let out = tempfile::tempdir().unwrap();
    let cfg = Config::new().with_out_dir(out.path());
    for export in hengst_desktop_lib::dto::ALL_TYPES {
        export(&cfg).unwrap();
    }
    let generated = read_dir(out.path());
    if std::env::var_os("HENGST_UPDATE_BINDINGS").is_some() {
        let _ = std::fs::remove_dir_all(BINDINGS);
        std::fs::create_dir_all(BINDINGS).unwrap();
        for (name, text) in &generated {
            std::fs::write(Path::new(BINDINGS).join(name), text).unwrap();
        }
        return;
    }
    let checked_in = read_dir(Path::new(BINDINGS));
    assert_eq!(
        checked_in.keys().collect::<Vec<_>>(),
        generated.keys().collect::<Vec<_>>(),
        "file set differs; run with HENGST_UPDATE_BINDINGS=1"
    );
    for (name, text) in &generated {
        assert_eq!(
            &checked_in[name], text,
            "{name} is stale; run with HENGST_UPDATE_BINDINGS=1"
        );
    }
}

/// The names passed to `invoke("<name>"` in a source text.
fn invoked(source: &str) -> Vec<String> {
    let mut names = vec![];
    for part in source.split("invoke").skip(1) {
        let mut rest = part;
        if rest.starts_with('<') {
            let mut depth = 0;
            let end = rest
                .char_indices()
                .find(|&(_, c)| {
                    match c {
                        '<' => depth += 1,
                        '>' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map_or(0, |(i, _)| i + 1);
            rest = &rest[end..];
        }
        if let Some(rest) = rest.strip_prefix("(\"") {
            if let Some(end) = rest.find('"') {
                names.push(rest[..end].to_string());
            }
        }
    }
    names
}

#[test]
fn ts_client_covers_every_command() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc");
    let mut found = std::collections::BTreeSet::new();
    for file in ["client.ts", "events.ts"] {
        let text = std::fs::read_to_string(Path::new(src).join(file)).unwrap();
        found.extend(invoked(&text));
    }
    let expected: std::collections::BTreeSet<String> = hengst_desktop_lib::COMMANDS
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn the_invoke_scanner_reads_plain_and_generic_calls() {
    assert_eq!(
        invoked("import { invoke } from 'x'; invoke(\"a\"); invoke<Promise<X>>(\"b\", {})"),
        ["a", "b"]
    );
}
