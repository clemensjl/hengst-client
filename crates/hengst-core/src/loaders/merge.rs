//! Merges a loader's child version JSON onto the vanilla parent version.

use crate::args::LEGACY_JVM;
use crate::paths::validate_id;
use crate::version::{Argument, Arguments, VersionJson};
use crate::{Error, Result};

/// Identity of a library for replacement: `group:artifact[:classifier]`, without version and
/// without an `@ext` suffix.
pub fn library_key(name: &str) -> String {
    let coord = name.split('@').next().unwrap_or(name);
    let mut parts = coord.split(':');
    let group = parts.next().unwrap_or("");
    let artifact = parts.next().unwrap_or("");
    let _version = parts.next();
    match parts.next() {
        Some(classifier) => format!("{group}:{artifact}:{classifier}"),
        None => format!("{group}:{artifact}"),
    }
}

/// Applies `child` (a loader profile) on top of `parent` (the vanilla version it inherits from).
pub fn merge_version(parent: &VersionJson, child: &VersionJson) -> Result<VersionJson> {
    if let Some(found) = &child.inherits_from {
        if *found != parent.id {
            return Err(Error::ParentMismatch {
                expected: parent.id.clone(),
                found: found.clone(),
            });
        }
    }
    validate_id(&child.id)?;

    let arguments = match (&parent.arguments, &child.arguments) {
        (None, None) => None,
        (Some(p), None) => Some(p.clone()),
        (Some(p), Some(c)) => Some(Arguments {
            game: p.game.iter().chain(&c.game).cloned().collect(),
            jvm: p.jvm.iter().chain(&c.jvm).cloned().collect(),
        }),
        (None, Some(c)) if c.jvm.is_empty() => Some(c.clone()),
        (None, Some(c)) => Some(Arguments {
            game: c.game.clone(),
            jvm: LEGACY_JVM
                .iter()
                .map(|s| Argument::Plain(s.to_string()))
                .chain(c.jvm.iter().cloned())
                .collect(),
        }),
    };

    let child_keys: Vec<String> = child
        .libraries
        .iter()
        .map(|l| library_key(&l.name))
        .collect();
    let libraries = child
        .libraries
        .iter()
        .cloned()
        .chain(
            parent
                .libraries
                .iter()
                .filter(|l| !child_keys.contains(&library_key(&l.name)))
                .cloned(),
        )
        .collect();

    let logging = match &child.logging {
        Some(l) if l.client.is_some() => Some(l.clone()),
        _ => parent.logging.clone(),
    };

    Ok(VersionJson {
        id: child.id.clone(),
        kind: child.kind.clone().or_else(|| parent.kind.clone()),
        main_class: child.main_class.clone(),
        minecraft_arguments: child
            .minecraft_arguments
            .clone()
            .or_else(|| parent.minecraft_arguments.clone()),
        arguments,
        asset_index: parent.asset_index.clone(),
        assets: parent.assets.clone(),
        downloads: parent.downloads.clone(),
        java_version: parent.java_version.clone(),
        libraries,
        logging,
        inherits_from: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn vj(v: Value) -> VersionJson {
        serde_json::from_value(v).unwrap()
    }

    fn lib(name: &str) -> Value {
        json!({"name": name})
    }

    fn plain(a: &Argument) -> &str {
        match a {
            Argument::Plain(s) => s,
            _ => panic!("expected plain argument"),
        }
    }

    fn modern_parent() -> VersionJson {
        vj(json!({
            "id": "1.21.11", "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {"jvm": ["-cp", "${classpath}"],
                "game": ["--username", "${auth_player_name}"]},
            "assetIndex": {"id": "29", "sha1": "a", "size": 1, "url": "http://x/29.json"},
            "assets": "29",
            "downloads": {"client": {"sha1": "c", "size": 3, "url": "http://x/client.jar"}},
            "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
            "libraries": [lib("com.mojang:brigadier:1.3.10"), lib("org.lwjgl:lwjgl:3.3.3"),
                lib("org.lwjgl:lwjgl:3.3.3:natives-windows")],
            "logging": {"client": {"argument": "-Dlog=${path}",
                "file": {"id": "client.xml", "sha1": "l", "size": 5, "url": "http://x/log.xml"}}}
        }))
    }

    fn legacy_parent(id: &str) -> VersionJson {
        vj(json!({
            "id": id, "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name}",
            "assetIndex": {"id": "1.8", "sha1": "a", "size": 1, "url": "http://x/1.8.json"},
            "libraries": [],
            "logging": {"client": {"argument": "-Dlog=${path}",
                "file": {"id": "client.xml", "sha1": "l", "size": 5, "url": "http://x/log.xml"}}}
        }))
    }

    #[test]
    fn library_key_ignores_version_and_extension() {
        assert_eq!(
            library_key("org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.17"),
            "org.lwjgl.lwjgl:lwjgl"
        );
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.3.3:natives-windows"),
            "org.lwjgl:lwjgl:natives-windows"
        );
        assert_eq!(
            library_key("de.oceanlabs.mcp:mcp_config:1.12.2-20200226.224830@zip"),
            "de.oceanlabs.mcp:mcp_config"
        );
    }

    #[test]
    fn merges_fabric_profile_onto_modern_parent() {
        let child = vj(json!({
            "id": "fabric-loader-0.17.3-1.21.11", "inheritsFrom": "1.21.11", "type": "release",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {"jvm": ["-XX:+UnlockDiagnosticVMOptions", "-XX:-UseBASE64Intrinsics",
                "-DFabricMcEmu= net.minecraft.client.main.Main "], "game": []},
            "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 17},
            "libraries": [
                {"name": "org.ow2.asm:asm:9.9", "url": "https://maven.fabricmc.net/",
                 "sha1": "c29635c8a7afa03d74b33c1884df8abb2b3f3dcc"},
                {"name": "net.fabricmc:fabric-loader:0.17.3", "url": "https://maven.fabricmc.net/"}]
        }));
        let m = merge_version(&modern_parent(), &child).unwrap();
        assert_eq!(m.id, "fabric-loader-0.17.3-1.21.11");
        assert_eq!(
            m.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert!(m.inherits_from.is_none());
        let args = m.arguments.as_ref().unwrap();
        assert_eq!(args.jvm.len(), 5);
        assert!(matches!(&args.jvm[0], Argument::Plain(s) if s == "-cp"));
        assert!(matches!(&args.jvm[4], Argument::Plain(s) if s.starts_with("-DFabricMcEmu=")));
        assert_eq!(args.game.len(), 2);
        let names: Vec<_> = m.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names[..2],
            ["org.ow2.asm:asm:9.9", "net.fabricmc:fabric-loader:0.17.3"]
        );
        assert_eq!(names.len(), 5);
        assert_eq!(m.java_version.unwrap().major_version, 21);
        assert_eq!(m.asset_index.unwrap().id, "29");
        assert!(m.downloads.unwrap().client.is_some());
        assert!(m.logging.unwrap().client.is_some());
    }

    #[test]
    fn child_libraries_replace_parent_by_key() {
        let parent = vj(json!({
            "id": "1.8.9", "mainClass": "net.minecraft.client.main.Main",
            "libraries": [
                {"name": "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209",
                 "rules": [{"action": "allow"}, {"action": "disallow", "os": {"name": "osx"}}]},
                {"name": "org.lwjgl.lwjgl:lwjgl:2.9.2-nightly-20140822",
                 "rules": [{"action": "allow", "os": {"name": "osx"}}]},
                lib("org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209"),
                lib("com.google.guava:guava:17.0")]
        }));
        let child = vj(json!({
            "id": "fabric-loader-0.17.3-1.8.9", "inheritsFrom": "1.8.9",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [lib("org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.17"),
                lib("org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.17")]
        }));
        let m = merge_version(&parent, &child).unwrap();
        let names: Vec<_> = m.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.17",
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.17",
                "com.google.guava:guava:17.0"
            ]
        );
    }

    #[test]
    fn legacy_forge_replaces_minecraft_arguments_and_keeps_parent_logging() {
        let child = vj(json!({
            "id": "1.12.2-forge-14.23.5.2860", "inheritsFrom": "1.12.2",
            "mainClass": "net.minecraft.launchwrapper.Launch",
            "minecraftArguments": "--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker --versionType Forge",
            "logging": {}
        }));
        let m = merge_version(&legacy_parent("1.12.2"), &child).unwrap();
        assert!(m
            .minecraft_arguments
            .unwrap()
            .ends_with("--versionType Forge"));
        assert!(m.arguments.is_none());
        assert!(m.logging.unwrap().client.is_some());
    }

    #[test]
    fn child_jvm_args_on_legacy_parent_get_legacy_defaults_first() {
        let child = vj(json!({
            "id": "x-1.12.2", "inheritsFrom": "1.12.2", "mainClass": "a.B",
            "arguments": {"jvm": ["-Dx=1"]}
        }));
        let m = merge_version(&legacy_parent("1.12.2"), &child).unwrap();
        let jvm: Vec<String> = m
            .arguments
            .unwrap()
            .jvm
            .iter()
            .map(|a| plain(a).to_string())
            .collect();
        let mut expected: Vec<String> = LEGACY_JVM.iter().map(|s| s.to_string()).collect();
        expected.push("-Dx=1".into());
        assert_eq!(jvm, expected);
        assert!(m.minecraft_arguments.is_some());
    }

    #[test]
    fn rejects_profile_for_other_parent() {
        let child = vj(json!({
            "id": "fabric-loader-0.17.3-1.21.10", "inheritsFrom": "1.21.10", "mainClass": "a.B"
        }));
        assert!(matches!(
            merge_version(&modern_parent(), &child),
            Err(Error::ParentMismatch { expected, found })
                if expected == "1.21.11" && found == "1.21.10"
        ));
    }

    #[test]
    fn rejects_unsafe_merged_id() {
        let child = vj(json!({
            "id": "../evil", "inheritsFrom": "1.21.11", "mainClass": "a.B"
        }));
        assert!(matches!(
            merge_version(&modern_parent(), &child),
            Err(Error::UnsafePath(_))
        ));
    }
}
