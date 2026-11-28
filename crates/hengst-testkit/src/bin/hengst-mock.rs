//! Starts the mock services for end-to-end tests: prints `{"url":"http://127.0.0.1:<port>"}` as
//! its first stdout line and serves until stdin closes. The scenario comes from
//! `HENGST_MOCK_SCENARIO` (default `ok`; names in `AuthScenario::from_name`).
//! `HENGST_MOCK_MOJANG_DELAY_MS` (default 0) makes every Mojang and Fabric answer wait that long,
//! so a game's preparation lasts long enough to watch or cancel.

use hengst_testkit::{AuthScenario, Services};
use std::io::{Read, Write};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let name = std::env::var("HENGST_MOCK_SCENARIO").unwrap_or_else(|_| "ok".into());
    let Some(scenario) = AuthScenario::from_name(&name) else {
        eprintln!("hengst-mock: unknown scenario {name:?}");
        std::process::exit(2);
    };
    let delay = match std::env::var("HENGST_MOCK_MOJANG_DELAY_MS") {
        Err(_) => Duration::ZERO,
        Ok(ms) => match ms.parse() {
            Ok(ms) => Duration::from_millis(ms),
            Err(_) => {
                eprintln!("hengst-mock: HENGST_MOCK_MOJANG_DELAY_MS is not a number: {ms:?}");
                std::process::exit(2);
            }
        },
    };
    let services = Services::start_with(scenario, delay).await;
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
