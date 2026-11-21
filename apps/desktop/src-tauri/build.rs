use std::path::Path;
use tauri_build::{AppManifest, Attributes};

include!("src/command_list.rs");

macro_rules! names {
    ($($m:ident :: $n:ident),* $(,)?) => {
        &[$(stringify!($n)),*]
    };
}

fn main() {
    // `tauri::generate_context!` embeds `frontendDist` at compile time and needs the folder to
    // exist. A plain `cargo build/test/clippy` must not need pnpm, so leave a placeholder; a real
    // `pnpm build` replaces it.
    let dist = Path::new("../dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).expect("create ../dist");
        std::fs::write(
            dist.join("index.html"),
            "Front end not built: run pnpm build in apps/desktop\n",
        )
        .expect("write placeholder index.html");
    }
    // The test executables (unit and integration) link the Tauri runtime, which needs Common
    // Controls v6; without a manifest they fail to load (STATUS_ENTRYPOINT_NOT_FOUND). Cargo has no
    // link argument for unit tests only, so every target gets the manifest and the app binary
    // opts out below.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{manifest}/windows-test.manifest");
        // The app binary already carries tauri-build's manifest as a resource; embedding a second
        // one is a link error.
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
        println!("cargo:rustc-link-arg-bins=/IGNORE:4075");
        println!("cargo:rerun-if-changed=windows-test.manifest");
    }
    println!("cargo:rerun-if-changed=src/command_list.rs");
    println!("cargo:rerun-if-changed=../dist");
    tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(with_commands!(names))),
    )
    .expect("failed to run tauri-build");
}
