use crate::download::Download;
use crate::paths::{join_relative, Paths};
use crate::rules::{rules_allow, Env};
use crate::version::Library;
use crate::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeJar {
    pub path: PathBuf,
    pub exclude: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ResolvedLibraries {
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<NativeJar>,
    pub downloads: Vec<Download>,
}

/// `group:artifact:version[:classifier][@ext]` to a repository-relative path.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!(
        "{}/{artifact}/{version}/{file}",
        group.replace('.', "/")
    ))
}

fn is_native_classifier(name: &str) -> bool {
    let coords = name.split('@').next().unwrap_or(name);
    coords
        .split(':')
        .nth(3)
        .is_some_and(|c| c.starts_with("natives-"))
}

/// Architecture a `natives-*` classifier is built for. Mojang's rules only carry the OS for these
/// (e.g. all three `natives-windows*` LWJGL jars allow "windows"), so the suffix decides.
fn native_classifier_arch(name: &str) -> Option<&'static str> {
    let coords = name.split('@').next().unwrap_or(name);
    let classifier = coords.split(':').nth(3)?;
    if !classifier.starts_with("natives-") {
        return None;
    }
    Some(
        if classifier.ends_with("-arm64") || classifier.ends_with("-aarch64") {
            "arm64"
        } else if classifier.ends_with("-x86") {
            "x86"
        } else if classifier.ends_with("-arm32") {
            "arm32"
        } else {
            "x86_64"
        },
    )
}

fn base_url(lib: &Library) -> String {
    let base = lib.url.clone().unwrap_or_else(|| LIBRARIES_URL.to_string());
    if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    }
}

pub(crate) fn resolve(libs: &[Library], env: &Env, paths: &Paths) -> Result<ResolvedLibraries> {
    let lib_dir = paths.libraries();
    let mut out = ResolvedLibraries::default();
    let mut queued = HashSet::new();
    let mut push = |out: &mut ResolvedLibraries, d: Download| {
        if queued.insert(d.path.clone()) {
            out.downloads.push(d);
        }
    };

    for lib in libs {
        if !rules_allow(&lib.rules, env) || lib.clientreq == Some(false) {
            continue;
        }
        if native_classifier_arch(&lib.name).is_some_and(|arch| arch != env.arch) {
            continue;
        }
        let exclude = lib.extract.clone().unwrap_or_default().exclude;

        // Pre-1.19 style: natives live in a classifier selected per OS.
        if let Some(classifier) = lib.natives.as_ref().and_then(|n| n.get(&env.os_name)) {
            let classifier = classifier.replace("${arch}", env.arch_bits());
            let artifact = lib
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.as_ref())
                .and_then(|c| c.get(&classifier));
            if lib.downloads.is_none() {
                // Maven-style natives (Legacy Fabric): only the classifier jar exists; its hash
                // comes from the `.sha1` sidecar.
                let coords = lib.name.split('@').next().unwrap_or(&lib.name);
                let rel = maven_path(&format!("{coords}:{classifier}"))
                    .ok_or_else(|| crate::Error::UnsafePath(lib.name.clone()))?;
                let path = join_relative(&lib_dir, &rel)?;
                push(
                    &mut out,
                    Download {
                        url: format!("{}{rel}", base_url(lib)),
                        path: path.clone(),
                        sha1: None,
                        sha1_from_cache: false,
                    },
                );
                out.natives.push(NativeJar {
                    path,
                    exclude: exclude.clone(),
                });
            } else if let Some(a) = artifact {
                let path = join_relative(&lib_dir, &a.path)?;
                push(
                    &mut out,
                    Download {
                        url: a.url.clone(),
                        path: path.clone(),
                        sha1: Some(a.sha1.clone()),
                        sha1_from_cache: false,
                    },
                );
                out.natives.push(NativeJar {
                    path,
                    exclude: exclude.clone(),
                });
            }
        }

        // A natives map without `downloads` has no main jar, only the classifier jar above.
        if lib.natives.is_some() && lib.downloads.is_none() {
            continue;
        }
        let artifact = match &lib.downloads {
            Some(d) => d
                .artifact
                .as_ref()
                .map(|a| (a.path.clone(), a.url.clone(), Some(a.sha1.clone()))),
            None => maven_path(&lib.name).map(|rel| {
                let sha1 = lib.sha1.clone().or_else(|| lib.checksums.first().cloned());
                (rel.clone(), format!("{}{rel}", base_url(lib)), sha1)
            }),
        };
        if let Some((rel, url, sha1)) = artifact {
            let path = join_relative(&lib_dir, &rel)?;
            // Empty url: the installer provides the jar. It is on the classpath, never downloaded.
            if !url.is_empty() {
                push(
                    &mut out,
                    Download {
                        url,
                        path: path.clone(),
                        sha1,
                        sha1_from_cache: false,
                    },
                );
            }
            if is_native_classifier(&lib.name) {
                out.natives.push(NativeJar {
                    path: path.clone(),
                    exclude: exclude.clone(),
                });
            }
            if !out.classpath.contains(&path) {
                out.classpath.push(path);
            }
        }
    }
    Ok(out)
}

fn is_native_file(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [".dll", ".so", ".dylib", ".jnilib"]
        .iter()
        .any(|ext| n.ends_with(ext))
}

/// Extracts native libraries flat into `dest` (only file names, so no path traversal).
pub(crate) fn extract_natives(natives: &[NativeJar], dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for jar in natives {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar.path)?)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            // zip 9: Namen mit ungueltigem UTF-8 liefern Err, die sind keine Natives.
            let Ok(name) = entry.name().map(|n| n.into_owned()) else {
                continue;
            };
            if name.starts_with("META-INF/")
                || jar.exclude.iter().any(|e| name.starts_with(e.as_str()))
            {
                continue;
            }
            if !is_native_file(&name) {
                continue;
            }
            let Some(file_name) = Path::new(&name).file_name() else {
                continue;
            };
            let mut out = std::fs::File::create(dest.join(file_name))?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn libs(v: serde_json::Value) -> Vec<Library> {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.16.0").unwrap(),
            "net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(maven_path("a.b:c:1@zip").unwrap(), "a/b/c/1/c-1.zip");
        assert!(maven_path("broken").is_none());
    }

    #[test]
    fn resolves_artifacts_natives_rules_and_maven_fallback() {
        let paths = Paths::new("C:/h");
        let list = libs(json!([
            {"name": "com.mojang:brigadier:1.0",
             "downloads": {"artifact": {"path": "com/mojang/brigadier/1.0/brigadier-1.0.jar", "sha1": "a1", "size": 1, "url": "http://x/brig.jar"}}},
            {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
             "natives": {"windows": "natives-windows-${arch}", "osx": "natives-osx"},
             "extract": {"exclude": ["META-INF/"]},
             "downloads": {"classifiers": {"natives-windows-64": {"path": "lwjgl-platform-natives-windows-64.jar", "sha1": "n1", "size": 1, "url": "http://x/n64.jar"}}}},
            {"name": "ca.weblite:java-objc-bridge:1.0",
             "rules": [{"action": "allow", "os": {"name": "osx"}}],
             "downloads": {"artifact": {"path": "objc.jar", "sha1": "o", "size": 1, "url": "http://x/objc.jar"}}},
            {"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
             "rules": [{"action": "allow", "os": {"name": "windows"}}],
             "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar", "sha1": "m", "size": 1, "url": "http://x/m.jar"}}},
            {"name": "net.fabricmc:fabric-loader:0.16.0", "url": "https://maven.fabricmc.net"}
        ]));
        let r = resolve(&list, &Env::windows_x64(), &paths).unwrap();
        let lib = paths.libraries();
        assert_eq!(
            r.classpath,
            vec![
                lib.join("com/mojang/brigadier/1.0/brigadier-1.0.jar"),
                lib.join("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"),
                lib.join("net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar"),
            ]
        );
        assert_eq!(r.natives.len(), 2);
        assert_eq!(
            r.natives[0].path,
            lib.join("lwjgl-platform-natives-windows-64.jar")
        );
        assert_eq!(r.natives[0].exclude, vec!["META-INF/".to_string()]);
        let fabric = r
            .downloads
            .iter()
            .find(|d| d.path.ends_with("fabric-loader-0.16.0.jar"))
            .unwrap();
        assert_eq!(
            fabric.url,
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar"
        );
        assert_eq!(fabric.sha1, None);
        assert!(!r.downloads.iter().any(|d| d.url.contains("objc")));
        assert_eq!(r.downloads.len(), 4);
    }

    #[test]
    fn resolves_legacy_fabric_maven_natives_without_main_jar() {
        let list = libs(json!([
            {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.17", "url": "https://maven.legacyfabric.net/",
             "extract": {"exclude": ["META-INF/"]},
             "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"}},
            {"name": "net.fabricmc:fabric-loader:0.20.1", "url": "https://maven.fabricmc.net/"},
            {"name": "org.ow2.asm:asm:9.10.1", "url": "https://maven.fabricmc.net/", "sha1": "ada2141c0cc52ee8f5c48cd5fa4ce0e794f22236"}
        ]));
        let r = resolve(&list, &Env::windows_x64(), &Paths::new("C:/h")).unwrap();
        let rel = "org/lwjgl/lwjgl/lwjgl-platform/2.9.4+legacyfabric.17/lwjgl-platform-2.9.4+legacyfabric.17-natives-windows.jar";
        assert_eq!(r.natives.len(), 1);
        assert_eq!(r.natives[0].path, Paths::new("C:/h").libraries().join(rel));
        assert_eq!(r.natives[0].exclude, vec!["META-INF/".to_string()]);
        assert!(r
            .downloads
            .iter()
            .any(|d| d.url == format!("https://maven.legacyfabric.net/{rel}")));
        assert!(!r
            .classpath
            .iter()
            .any(|p| p.to_string_lossy().contains("lwjgl-platform")));
        assert_eq!(r.classpath.len(), 2);
        let asm = r
            .downloads
            .iter()
            .find(|d| d.url.ends_with("asm-9.10.1.jar"))
            .unwrap();
        assert_eq!(
            asm.sha1.as_deref(),
            Some("ada2141c0cc52ee8f5c48cd5fa4ce0e794f22236")
        );
    }

    #[test]
    fn legacy_forge_checksums_clientreq_and_installer_provided_jars() {
        let list = libs(json!([
            {"name": "jline:jline:2.13", "url": "https://maven.minecraftforge.net/",
             "checksums": ["2d9530d0a25daffaffda7c35037b046b627bb171"], "serverreq": true, "clientreq": false},
            {"name": "org.scala-lang:scala-xml_2.11:1.0.2", "url": "https://maven.minecraftforge.net/",
             "checksums": ["7a80ec00aec122fba7cd4e0d4cdd87ff7e4cb6d0", "62736b01689d56b6d09a0164b7ef9da2b0b9633d"]},
            {"name": "net.minecraftforge:forge:1.16.5-36.2.34", "downloads": {"artifact": {
             "path": "net/minecraftforge/forge/1.16.5-36.2.34/forge-1.16.5-36.2.34.jar", "url": "",
             "sha1": "b05f056824252928b59c746f8a28fe2cf36db6c0", "size": 212608}}}
        ]));
        let r = resolve(&list, &Env::windows_x64(), &Paths::new("C:/h")).unwrap();
        assert_eq!(r.downloads.len(), 1);
        assert_eq!(
            r.downloads[0].sha1.as_deref(),
            Some("7a80ec00aec122fba7cd4e0d4cdd87ff7e4cb6d0")
        );
        assert_eq!(r.classpath.len(), 2);
        assert!(r.classpath[1].ends_with("forge-1.16.5-36.2.34.jar"));
    }

    #[test]
    fn rejects_maven_coordinates_that_escape() {
        let list = libs(json!([{"name": "..:evil:1", "url": "https://x/"}]));
        assert!(matches!(
            resolve(&list, &Env::windows_x64(), &Paths::new("C:/h")),
            Err(crate::Error::UnsafePath(_))
        ));
    }

    #[test]
    fn skips_natives_of_other_architectures() {
        let paths = Paths::new("C:/h");
        let native = |classifier: &str, os: &str| {
            let file = format!("lwjgl-3.3.3-{classifier}.jar");
            json!({"name": format!("org.lwjgl:lwjgl:3.3.3:{classifier}"),
             "rules": [{"action": "allow", "os": {"name": os}}],
             "downloads": {"artifact": {"path": format!("org/lwjgl/lwjgl/3.3.3/{file}"), "sha1": "s", "size": 1, "url": format!("http://x/{file}")}}})
        };
        let list = libs(json!([
            native("natives-windows", "windows"),
            native("natives-windows-arm64", "windows"),
            native("natives-windows-x86", "windows"),
            native("natives-linux", "linux"),
            native("natives-linux-arm64", "linux"),
            native("natives-macos", "osx"),
            native("natives-macos-arm64", "osx"),
        ]));
        let r = resolve(&list, &Env::windows_x64(), &paths).unwrap();
        let expected = paths
            .libraries()
            .join("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar");
        assert_eq!(r.classpath, vec![expected.clone()]);
        assert_eq!(r.natives.len(), 1);
        assert_eq!(r.natives[0].path, expected);
        assert_eq!(r.downloads.len(), 1);
        assert_eq!(r.downloads[0].path, expected);
    }

    #[test]
    fn rejects_library_paths_that_escape() {
        let list = libs(json!([
            {"name": "evil:lib:1",
             "downloads": {"artifact": {"path": "../../evil.jar", "sha1": "s", "size": 1, "url": "http://x/e.jar"}}}
        ]));
        let err = resolve(&list, &Env::windows_x64(), &Paths::new("C:/h")).unwrap_err();
        assert!(matches!(err, crate::Error::UnsafePath(_)));
    }

    #[test]
    fn extracts_only_native_files_flat() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("n.jar");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            for (name, body) in [
                ("lwjgl64.dll", "a"),
                ("windows/x64/org/lwjgl/glfw.dll", "b"),
                ("META-INF/MANIFEST.MF", "c"),
                ("skip/me.dll", "d"),
                ("readme.txt", "e"),
            ] {
                zip.start_file(name, opts).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let out = dir.path().join("natives");
        extract_natives(
            &[NativeJar {
                path: jar,
                exclude: vec!["skip/".into()],
            }],
            &out,
        )
        .unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["glfw.dll", "lwjgl64.dll"]);
    }
}
