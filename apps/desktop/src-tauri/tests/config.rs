fn config() -> serde_json::Value {
    let text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn directive(csp: &str, name: &str) -> String {
    csp.split(';')
        .map(str::trim)
        .find(|d| d.starts_with(&format!("{name} ")))
        .unwrap_or("")
        .to_string()
}

#[test]
fn csp_forbids_unsafe_inline_for_scripts_and_style_elements() {
    let conf = config();
    let csp = conf["app"]["security"]["csp"].as_str().unwrap();
    assert!(!directive(csp, "script-src").contains("'unsafe-inline'"));
    assert!(!directive(csp, "script-src").contains("'unsafe-eval'"));
    assert!(!directive(csp, "style-src").contains("'unsafe-inline'"));
    assert!(csp.contains("connect-src ipc: http://ipc.localhost"));
    assert_eq!(conf["app"]["security"]["freezePrototype"], true);
}

#[test]
fn script_sources_never_include_remote_origins_wildcards_or_eval() {
    let conf = config();
    for key in ["csp", "devCsp"] {
        let csp = conf["app"]["security"][key].as_str().unwrap();
        let scripts = directive(csp, "script-src");
        assert!(scripts.starts_with("script-src 'self'"), "{key}: {scripts}");
        for forbidden in ["https:", "http:", "*", "'unsafe-eval'", "data:", "blob:"] {
            assert!(
                !scripts.contains(forbidden),
                "{key}: {forbidden} in {scripts}"
            );
        }
        for source in scripts.split_whitespace().skip(1) {
            assert!(
                source == "'self'" || source == "'unsafe-inline'",
                "{key}: unexpected script source {source}"
            );
        }
        assert!(
            !directive(csp, "style-src").contains("'unsafe-inline'"),
            "{key}"
        );
    }
    // Only the dev server's React-refresh preamble may be inline, and only in devCsp.
    let csp = conf["app"]["security"]["csp"].as_str().unwrap();
    assert!(!directive(csp, "script-src").contains("'unsafe-inline'"));
}
