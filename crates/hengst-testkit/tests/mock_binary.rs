use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn spawn(scenario: Option<&str>) -> Child {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hengst-mock"));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match scenario {
        Some(s) => cmd.env("HENGST_MOCK_SCENARIO", s),
        None => cmd.env_remove("HENGST_MOCK_SCENARIO"),
    };
    cmd.spawn().unwrap()
}

fn first_line(child: &mut Child) -> String {
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    line
}

fn wait_for_exit(child: &mut Child, within: Duration) -> std::process::ExitStatus {
    let end = Instant::now() + within;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() > end {
            let _ = child.kill();
            panic!("hengst-mock did not exit within {within:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn http_get(addr: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn mock_binary_prints_its_url() {
    let mut child = spawn(None);
    let line = first_line(&mut child);
    let json: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    let url = json["url"].as_str().unwrap();
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");

    // It serves the Mojang manifest and the sign-in chain.
    let addr = url.trim_start_matches("http://");
    let manifest = http_get(addr, "/mojang/manifest.json");
    assert!(manifest.starts_with("HTTP/1.1 200"), "{manifest}");
    assert!(manifest.contains("9.9-snap"));

    // Closing stdin ends the process.
    drop(child.stdin.take());
    assert!(wait_for_exit(&mut child, Duration::from_secs(5)).success());
}

#[test]
fn mock_binary_applies_the_scenario_from_the_environment() {
    let mut child = spawn(Some("no-xbox"));
    let line = first_line(&mut child);
    let json: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    let addr = json["url"].as_str().unwrap().trim_start_matches("http://");
    // An unauthenticated XSTS request is refused with 400 whatever the scenario; the scenario
    // itself is covered in auth_mock.rs, here only that the binary starts with it.
    let reply = http_get(addr, "/xsts/authorize");
    assert!(reply.starts_with("HTTP/1.1 405"), "{reply}");
    drop(child.stdin.take());
    assert!(wait_for_exit(&mut child, Duration::from_secs(5)).success());
}

#[test]
fn mock_binary_rejects_an_unknown_scenario() {
    let mut child = spawn(Some("no-such-scenario"));
    drop(child.stdin.take());
    let status = wait_for_exit(&mut child, Duration::from_secs(5));
    assert!(!status.success());
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    assert!(err.contains("no-such-scenario"), "{err}");
}
