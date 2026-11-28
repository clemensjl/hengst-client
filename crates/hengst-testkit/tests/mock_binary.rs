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
fn mock_binary_delays_the_mojang_answers_when_asked() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hengst-mock"));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("HENGST_MOCK_SCENARIO")
        .env("HENGST_MOCK_MOJANG_DELAY_MS", "400");
    let mut child = cmd.spawn().unwrap();
    let line = first_line(&mut child);
    let json: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    let addr = json["url"]
        .as_str()
        .unwrap()
        .trim_start_matches("http://")
        .to_string();
    for path in ["/mojang/manifest.json", "/mojang/files/client.jar"] {
        let started = Instant::now();
        let answer = http_get(&addr, path);
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(started.elapsed() >= Duration::from_millis(400), "{path}");
    }
    drop(child.stdin.take());
    assert!(wait_for_exit(&mut child, Duration::from_secs(5)).success());

    // Anything but a number of milliseconds is refused at start.
    let mut bad = Command::new(env!("CARGO_BIN_EXE_hengst-mock"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("HENGST_MOCK_MOJANG_DELAY_MS", "soon")
        .spawn()
        .unwrap();
    assert_eq!(
        wait_for_exit(&mut bad, Duration::from_secs(5)).code(),
        Some(2)
    );
}

#[test]
fn fake_java_bursts_numbered_lines_of_the_given_length() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("fake-java.txt"), "burst 3 40\nout done\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_fake-java"))
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    for (i, line) in lines[..3].iter().enumerate() {
        assert!(
            line.starts_with(&format!("burst-line {} ", i + 1)),
            "{line}"
        );
        assert_eq!(line.len(), 40, "{line}");
    }
    assert_eq!(lines[3], "done");
}

fn http_post_json(addr: &str, path: &str, body: &serde_json::Value) -> String {
    let body = body.to_string();
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "POST {path} HTTP/1.1
Host: {addr}
Content-Type: application/json
Content-Length: {}
Connection: close

{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).unwrap();
    out
}

/// A valid XSTS request for account A: the answer depends on the scenario.
fn xsts_answer(scenario: &str) -> String {
    let mut child = spawn(Some(scenario));
    let line = first_line(&mut child);
    let json: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    let addr = json["url"].as_str().unwrap().trim_start_matches("http://");
    let reply = http_post_json(
        addr,
        "/xsts/authorize",
        &serde_json::json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [hengst_testkit::PLAYER_A.xbox_user_token()]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    );
    drop(child.stdin.take());
    assert!(wait_for_exit(&mut child, Duration::from_secs(5)).success());
    reply
}

#[test]
fn mock_binary_applies_the_scenario_from_the_environment() {
    // The same valid request is answered differently depending on the scenario.
    let ok = xsts_answer("ok");
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("test-xsts-"), "{ok}");
    let no_xbox = xsts_answer("no-xbox");
    assert!(no_xbox.starts_with("HTTP/1.1 401"), "{no_xbox}");
    assert!(no_xbox.contains("2148916233"), "{no_xbox}");
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
