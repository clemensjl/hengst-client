#[test]
fn csp_forbids_unsafe_inline_for_scripts_and_style_elements() {
    let text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json")).unwrap();
    let conf: serde_json::Value = serde_json::from_str(&text).unwrap();
    let csp = conf["app"]["security"]["csp"].as_str().unwrap();
    let directive = |name: &str| {
        csp.split(';')
            .map(str::trim)
            .find(|d| d.starts_with(&format!("{name} ")))
            .unwrap_or("")
            .to_string()
    };
    assert!(!directive("script-src").contains("'unsafe-inline'"));
    assert!(!directive("script-src").contains("'unsafe-eval'"));
    assert!(!directive("style-src").contains("'unsafe-inline'"));
    assert!(csp.contains("connect-src ipc: http://ipc.localhost"));
    assert_eq!(conf["app"]["security"]["freezePrototype"], true);
}
