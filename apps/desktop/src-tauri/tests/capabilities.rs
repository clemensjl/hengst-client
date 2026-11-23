use std::collections::BTreeSet;

const EXTRA_PERMISSIONS: &[&str] = &[]; // core: permissions the event channel provably needs

fn allow_name(cmd: &str) -> String {
    format!("allow-{}", cmd.replace('_', "-"))
}

#[test]
fn capability_allows_exactly_the_commands() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/capabilities/main.json"
    ))
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let granted: BTreeSet<String> = json["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_string())
        .collect();
    let expected: BTreeSet<String> = hengst_desktop_lib::COMMANDS
        .iter()
        .map(|c| allow_name(c))
        .chain(EXTRA_PERMISSIONS.iter().map(|p| p.to_string()))
        .collect();
    assert_eq!(
        granted, expected,
        "every command needs one allow entry and no entry may name a missing command"
    );
    assert!(
        granted.iter().all(|p| !p.starts_with("opener:")),
        "opener is used from Rust only"
    );
}

#[test]
fn command_names_are_unique() {
    let set: BTreeSet<&&str> = hengst_desktop_lib::COMMANDS.iter().collect();
    assert_eq!(set.len(), hengst_desktop_lib::COMMANDS.len());
}
