//! The test kit and its mock server must never be part of a shipped binary. `cargo tree -e
//! normal` lists what a build of a package links (no dev or build dependencies); the CLI and the
//! desktop app, built the way a release is built, must not list either.
//!
//! `cargo test --workspace` runs this in CI. The desktop app's `e2e` feature (which pulls the
//! kit in, from Task 7 on) is excluded on purpose: it is a debug-only feature, and the app
//! refuses to compile it in a release build (`compile_error!` in `apps/desktop/src-tauri`).

use std::path::Path;
use std::process::Command;

const FORBIDDEN: [&str; 2] = ["hengst-testkit", "wiremock"];

fn tree(package: &str, features: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args([
        "tree",
        "--locked",
        "--offline",
        "-e",
        "normal",
        "--prefix",
        "none",
        "--format",
        "{p}",
    ])
    .arg("--manifest-path")
    .arg(&manifest)
    .args(["-p", package]);
    if !features.is_empty() {
        cmd.args(["--features", features]);
    }
    let out = cmd.output().expect("cargo tree runs");
    assert!(
        out.status.success(),
        "cargo tree -p {package} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn lists(tree: &str, package: &str) -> bool {
    tree.lines()
        .any(|l| l.split_whitespace().next() == Some(package))
}

#[test]
fn shipped_binaries_do_not_link_the_test_kit() {
    // The two release configurations of each binary.
    let builds = [
        ("hengst-cli", ""),
        ("hengst-cli", "dev-offline"),
        ("hengst-desktop", ""),
        ("hengst-desktop", "custom-protocol"),
        ("hengst-desktop", "custom-protocol,dev-offline"),
    ];
    for (package, features) in builds {
        let t = tree(package, features);
        // The check must not pass because the output is empty or in another shape.
        assert!(lists(&t, package), "{package}: {t}");
        assert!(lists(&t, "hengst-core"), "{package}: {t}");
        for forbidden in FORBIDDEN {
            assert!(
                !lists(&t, forbidden),
                "{package} (features: {features:?}) links {forbidden}"
            );
        }
    }
}

#[test]
fn the_check_sees_the_kit_where_it_is_a_normal_dependency() {
    // Control: for the kit itself the same query does list its dependencies.
    let t = tree("hengst-testkit", "");
    assert!(lists(&t, "hengst-testkit"));
    assert!(lists(&t, "wiremock"));
}
