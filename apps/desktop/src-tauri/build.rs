use std::path::Path;
use tauri_build::{AppManifest, Attributes, WindowsAttributes};

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
    // One manifest for every executable of this package (the app and the unit and integration
    // test executables): the Common Controls v6 dependency that tauri-build's default manifest
    // holds (nothing else), which the Tauri runtime needs to load at all. tauri-build is told not
    // to add its own copy below. Cargo has no link argument for unit tests only, hence the generic
    // `rustc-link-arg`.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{manifest}/windows.manifest");
        println!("cargo:rerun-if-changed=windows.manifest");
    }
    println!("cargo:rerun-if-changed=src/command_list.rs");
    println!("cargo:rerun-if-changed=../dist");
    tauri_build::try_build(
        Attributes::new()
            .windows_attributes(WindowsAttributes::new_without_app_manifest())
            .app_manifest(AppManifest::new().commands(with_commands!(names))),
    )
    .expect("failed to run tauri-build");
}
