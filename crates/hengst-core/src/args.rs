use crate::assets::AssetsLayout;
use crate::rules::{rules_allow, Env};
use crate::version::{ArgValue, Argument, VersionJson};
use md5::{Digest, Md5};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Account {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub xuid: String,
}

impl Account {
    /// Offline account for local development. UUID matches Java's
    /// `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)` like vanilla servers use.
    pub fn offline(name: &str) -> Self {
        let mut d: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
        d[6] = (d[6] & 0x0f) | 0x30;
        d[8] = (d[8] & 0x3f) | 0x80;
        Self {
            name: name.to_string(),
            uuid: hex::encode(d),
            access_token: "0".into(),
            user_type: "legacy".into(),
            xuid: "0".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaunchContext {
    pub account: Account,
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub libraries_dir: PathBuf,
    /// Libraries first, client jar last.
    pub classpath: Vec<PathBuf>,
    pub assets: AssetsLayout,
    pub log_config: Option<PathBuf>,
    pub env: Env,
    pub memory_mb: u32,
}

const WINDOWS_HEAPDUMP: &str =
    "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump";
const LEGACY_JVM: &[&str] = &[
    "-Djava.library.path=${natives_directory}",
    "-cp",
    "${classpath}",
];

/// Everything after the java executable: JVM args, main class, game args.
pub fn build_args(v: &VersionJson, ctx: &LaunchContext) -> Vec<String> {
    let vars = variables(v, ctx);
    let mut out = vec![
        format!("-Xmx{}M", ctx.memory_mb),
        format!("-Xms{}M", ctx.memory_mb.min(512)),
    ];
    match v.arguments.as_ref().filter(|a| !a.jvm.is_empty()) {
        Some(a) => out.extend(expand(&a.jvm, &ctx.env)),
        None => {
            if ctx.env.os_name == "windows" {
                out.push(WINDOWS_HEAPDUMP.to_string());
            }
            out.extend(LEGACY_JVM.iter().map(|s| s.to_string()));
        }
    }
    let log_arg = v
        .logging
        .as_ref()
        .and_then(|l| l.client.as_ref())
        .map(|c| &c.argument);
    if let (Some(path), Some(arg)) = (&ctx.log_config, log_arg) {
        out.push(arg.replace("${path}", &path.to_string_lossy()));
    }
    out.push(v.main_class.clone());
    match (&v.arguments, &v.minecraft_arguments) {
        (Some(a), _) if !a.game.is_empty() => out.extend(expand(&a.game, &ctx.env)),
        (_, Some(legacy)) => out.extend(legacy.split_whitespace().map(String::from)),
        _ => {}
    }
    out.into_iter().map(|s| substitute(&s, &vars)).collect()
}

fn expand(args: &[Argument], env: &Env) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(s.clone()),
            Argument::Conditional { rules, value } if rules_allow(rules, env) => match value {
                ArgValue::One(s) => out.push(s.clone()),
                ArgValue::Many(list) => out.extend(list.iter().cloned()),
            },
            Argument::Conditional { .. } => {}
        }
    }
    out
}

fn variables(v: &VersionJson, ctx: &LaunchContext) -> HashMap<&'static str, String> {
    let s = |p: &Path| p.to_string_lossy().into_owned();
    let sep = ctx.env.classpath_separator();
    let classpath = ctx
        .classpath
        .iter()
        .map(|p| s(p))
        .collect::<Vec<_>>()
        .join(sep);
    let a = &ctx.account;
    HashMap::from([
        ("auth_player_name", a.name.clone()),
        ("auth_uuid", a.uuid.clone()),
        ("auth_access_token", a.access_token.clone()),
        (
            "auth_session",
            format!("token:{}:{}", a.access_token, a.uuid),
        ),
        ("auth_xuid", a.xuid.clone()),
        ("clientid", "0".to_string()),
        ("user_type", a.user_type.clone()),
        ("user_properties", "{}".to_string()),
        ("version_name", v.id.clone()),
        (
            "version_type",
            v.kind.clone().unwrap_or_else(|| "release".into()),
        ),
        ("game_directory", s(&ctx.game_dir)),
        ("assets_root", s(&ctx.assets.assets_root)),
        ("game_assets", s(&ctx.assets.game_assets)),
        ("assets_index_name", ctx.assets.index_id.clone()),
        ("natives_directory", s(&ctx.natives_dir)),
        ("library_directory", s(&ctx.libraries_dir)),
        ("classpath", classpath),
        ("classpath_separator", sep.to_string()),
        ("launcher_name", "hengst-client".to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("resolution_width", "854".to_string()),
        ("resolution_height", "480".to_string()),
    ])
}

/// Replaces `${key}` with known values; unknown placeholders stay untouched.
pub fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(value) => out.push_str(value),
                    None => {
                        out.push_str("${");
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> LaunchContext {
        LaunchContext {
            account: Account::offline("Dev"),
            game_dir: PathBuf::from("C:/g"),
            natives_dir: PathBuf::from("C:/g/.natives/v"),
            libraries_dir: PathBuf::from("C:/h/libraries"),
            classpath: vec![PathBuf::from("C:/a.jar"), PathBuf::from("C:/client.jar")],
            assets: AssetsLayout {
                index_id: "29".into(),
                assets_root: PathBuf::from("C:/h/assets"),
                game_assets: PathBuf::from("C:/h/assets"),
            },
            log_config: Some(PathBuf::from("C:/h/assets/log_configs/client.xml")),
            env: Env::windows_x64(),
            memory_mb: 2048,
        }
    }

    #[test]
    fn offline_uuid_is_version_3_and_stable() {
        let a = Account::offline("Dev");
        assert_eq!(a.uuid.len(), 32);
        assert_eq!(&a.uuid[12..13], "3");
        assert!(matches!(&a.uuid[16..17], "8" | "9" | "a" | "b"));
        assert_eq!(a.uuid, Account::offline("Dev").uuid);
        assert_ne!(a.uuid, Account::offline("Other").uuid);
    }

    #[test]
    fn substitute_keeps_unknown_placeholders() {
        let vars = HashMap::from([("a", "1".to_string())]);
        assert_eq!(substitute("x${a}y${b}z${", &vars), "x1y${b}z${");
    }

    #[test]
    fn modern_arguments() {
        let v: VersionJson = serde_json::from_value(json!({
            "id": "1.21.11", "type": "release", "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
                "game": ["--username", "${auth_player_name}", "--assetIndex", "${assets_index_name}",
                    {"rules": [{"action": "allow", "features": {"is_quick_play_singleplayer": true}}], "value": ["--quickPlaySingleplayer", "${quickPlaySingleplayer}"]}],
                "jvm": [
                    {"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": "-XX:HeapDumpPath=dump"},
                    {"rules": [{"action": "allow", "os": {"arch": "x86"}}], "value": "-Xss1M"},
                    "-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]
            },
            "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "file": {"id": "client.xml", "sha1": "s", "size": 1, "url": "u"}}}
        })).unwrap();
        let args = build_args(&v, &ctx());
        let cp = args.iter().position(|a| a == "-cp").unwrap();
        assert_eq!(args[cp + 1], "C:/a.jar;C:/client.jar");
        assert!(args.contains(&"-Xmx2048M".to_string()));
        assert!(args.contains(&"-XX:HeapDumpPath=dump".to_string()));
        assert!(!args.contains(&"-Xss1M".to_string()));
        assert!(args.contains(&"-Djava.library.path=C:/g/.natives/v".to_string()));
        assert!(args
            .contains(&"-Dlog4j.configurationFile=C:/h/assets/log_configs/client.xml".to_string()));
        let main = args
            .iter()
            .position(|a| a == "net.minecraft.client.main.Main")
            .unwrap();
        assert!(main > cp);
        assert_eq!(
            &args[main + 1..],
            &["--username", "Dev", "--assetIndex", "29"]
        );
    }

    #[test]
    fn legacy_arguments_get_default_jvm_args() {
        let v: VersionJson = serde_json::from_value(json!({
            "id": "b1.7.3", "type": "old_beta", "mainClass": "net.minecraft.launchwrapper.Launch",
            "minecraftArguments": "${auth_player_name} ${auth_session} --gameDir ${game_directory} --assetsDir ${game_assets}"
        })).unwrap();
        let args = build_args(&v, &ctx());
        let a = Account::offline("Dev");
        assert!(args.contains(&"-Djava.library.path=C:/g/.natives/v".to_string()));
        assert!(args
            .iter()
            .any(|x| x.starts_with("-XX:HeapDumpPath=MojangTricksIntelDrivers")));
        let main = args
            .iter()
            .position(|x| x == "net.minecraft.launchwrapper.Launch")
            .unwrap();
        assert_eq!(
            &args[main + 1..],
            &[
                "Dev".to_string(),
                format!("token:0:{}", a.uuid),
                "--gameDir".into(),
                "C:/g".into(),
                "--assetsDir".into(),
                "C:/h/assets".into()
            ]
        );
    }
}
