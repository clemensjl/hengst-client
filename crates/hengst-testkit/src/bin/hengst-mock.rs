//! Starts the mock services for end-to-end tests: prints `{"url":"http://127.0.0.1:<port>"}` as
//! its first stdout line and serves until stdin closes. The scenario comes from
//! `HENGST_MOCK_SCENARIO` (default `ok`; names in `AuthScenario::from_name`).

use hengst_testkit::{AuthScenario, Services};
use std::io::{Read, Write};

#[tokio::main]
async fn main() {
    let name = std::env::var("HENGST_MOCK_SCENARIO").unwrap_or_else(|_| "ok".into());
    let Some(scenario) = AuthScenario::from_name(&name) else {
        eprintln!("hengst-mock: unknown scenario {name:?}");
        std::process::exit(2);
    };
    let services = Services::start(scenario).await;
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{}", serde_json::json!({"url": services.uri()}));
    let _ = out.flush();
    // Serve until the parent closes our stdin (or dies).
    let _ = tokio::task::spawn_blocking(|| {
        let mut sink = [0u8; 256];
        let mut stdin = std::io::stdin();
        while matches!(stdin.read(&mut sink), Ok(n) if n > 0) {}
    })
    .await;
}
