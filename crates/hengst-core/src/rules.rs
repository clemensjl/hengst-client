use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rule {
    pub action: Action,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

/// The machine the game runs on, in Mojang's vocabulary.
#[derive(Clone, Debug)]
pub struct Env {
    pub os_name: String,
    pub arch: String,
    pub os_version: String,
    pub features: HashMap<String, bool>,
}

impl Env {
    pub fn current() -> Self {
        let os_name = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            _ => "linux",
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "x86",
            "aarch64" => "arm64",
            other => other,
        };
        // Windows 10 and 11 both report 10.0 to Java, which is what Mojang's rules match against.
        let os_version = if os_name == "windows" { "10.0" } else { "" };
        Self {
            os_name: os_name.into(),
            arch: arch.into(),
            os_version: os_version.into(),
            features: HashMap::new(),
        }
    }

    pub fn windows_x64() -> Self {
        Self {
            os_name: "windows".into(),
            arch: "x86_64".into(),
            os_version: "10.0".into(),
            features: HashMap::new(),
        }
    }

    /// Value for `${arch}` in legacy native classifiers.
    pub fn arch_bits(&self) -> &'static str {
        if self.arch == "x86" {
            "32"
        } else {
            "64"
        }
    }

    pub fn classpath_separator(&self) -> &'static str {
        if self.os_name == "windows" {
            ";"
        } else {
            ":"
        }
    }
}

impl Rule {
    fn matches(&self, env: &Env) -> bool {
        if let Some(os) = &self.os {
            if os.name.as_ref().is_some_and(|n| n != &env.os_name) {
                return false;
            }
            if os.arch.as_ref().is_some_and(|a| a != &env.arch) {
                return false;
            }
            if let Some(v) = &os.version {
                match Regex::new(v) {
                    Ok(re) if re.is_match(&env.os_version) => {}
                    _ => return false,
                }
            }
        }
        if let Some(features) = &self.features {
            for (key, wanted) in features {
                if env.features.get(key).copied().unwrap_or(false) != *wanted {
                    return false;
                }
            }
        }
        true
    }
}

/// Mojang semantics: no rules means allowed; otherwise the last matching rule decides, default deny.
pub fn rules_allow(rules: &[Rule], env: &Env) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule.matches(env) {
            allowed = rule.action == Action::Allow;
        }
    }
    allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules(v: serde_json::Value) -> Vec<Rule> {
        serde_json::from_value(v).unwrap()
    }

    fn linux() -> Env {
        Env {
            os_name: "linux".into(),
            os_version: String::new(),
            ..Env::windows_x64()
        }
    }

    #[test]
    fn no_rules_allows() {
        assert!(rules_allow(&[], &Env::windows_x64()));
    }

    #[test]
    fn os_name_allow() {
        let r = rules(json!([{"action": "allow", "os": {"name": "windows"}}]));
        assert!(rules_allow(&r, &Env::windows_x64()));
        assert!(!rules_allow(&r, &linux()));
    }

    #[test]
    fn last_match_wins() {
        let r = rules(json!([
            {"action": "allow"},
            {"action": "disallow", "os": {"name": "osx"}}
        ]));
        assert!(rules_allow(&r, &Env::windows_x64()));
        let r = rules(json!([
            {"action": "allow"},
            {"action": "disallow", "os": {"name": "windows"}}
        ]));
        assert!(!rules_allow(&r, &Env::windows_x64()));
    }

    #[test]
    fn version_regex_and_arch() {
        let r = rules(json!([{"action": "allow", "os": {"name": "windows", "version": "^10\\."}}]));
        assert!(rules_allow(&r, &Env::windows_x64()));
        let r = rules(json!([{"action": "allow", "os": {"arch": "x86"}}]));
        assert!(!rules_allow(&r, &Env::windows_x64()));
        assert_eq!(Env::windows_x64().arch_bits(), "64");
    }

    #[test]
    fn features_default_false() {
        let r = rules(json!([{"action": "allow", "features": {"has_custom_resolution": true}}]));
        assert!(!rules_allow(&r, &Env::windows_x64()));
        let mut env = Env::windows_x64();
        env.features.insert("has_custom_resolution".into(), true);
        assert!(rules_allow(&r, &env));
    }
}
