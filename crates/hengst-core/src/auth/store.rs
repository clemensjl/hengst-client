//! Saved accounts: metadata in `accounts.json`, tokens in a secret store
//! (the Windows Credential Manager in the real launcher).

use super::flow::LoginSession;
use super::AuthError;
use crate::secret::Secret;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Service name of every Credential Manager entry.
pub const SERVICE: &str = "hengst-client";

const FILE_VERSION: u32 = 1;

/// Where confidential values live. `delete` of a missing key is `Ok`.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Secret>>;
    fn set(&self, key: &str, value: &Secret) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// In-memory store for tests.
#[derive(Default)]
pub struct MemorySecretStore {
    map: Mutex<HashMap<String, Secret>>,
}

impl SecretStore for MemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<Secret>> {
        Ok(self.map.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: &Secret) -> Result<()> {
        self.map
            .lock()
            .unwrap()
            .insert(key.to_string(), value.clone());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.map.lock().unwrap().remove(key);
        Ok(())
    }
}

/// Windows Credential Manager. Values are stored as UTF-8 blobs (limit 2560
/// bytes), not as UTF-16 passwords (which would halve the room).
#[cfg(windows)]
pub struct KeyringSecretStore {
    store: std::sync::Arc<keyring_core::api::CredentialStore>,
}

#[cfg(windows)]
impl KeyringSecretStore {
    pub fn new() -> Result<Self> {
        let store = windows_native_keyring_store::Store::new().map_err(credential_error)?;
        Ok(Self { store })
    }

    fn entry(&self, key: &str) -> Result<keyring_core::Entry> {
        keyring_core::Entry::new_in_store(&self.store, SERVICE, key).map_err(credential_error)
    }
}

/// Only the library's error text, never a value.
#[cfg(windows)]
fn credential_error(e: keyring_core::Error) -> Error {
    Error::CredentialStore(e.to_string())
}

#[cfg(windows)]
impl SecretStore for KeyringSecretStore {
    fn get(&self, key: &str) -> Result<Option<Secret>> {
        match self.entry(key)?.get_secret() {
            Ok(bytes) => String::from_utf8(bytes)
                .map(|s| Some(Secret::new(s)))
                .map_err(|_| Error::CredentialStore("stored value is not valid UTF-8".into())),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(credential_error(e)),
        }
    }

    fn set(&self, key: &str, value: &Secret) -> Result<()> {
        self.entry(key)?
            .set_secret(value.expose().as_bytes())
            .map_err(credential_error)
    }

    fn delete(&self, key: &str) -> Result<()> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(credential_error(e)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    Refresh,
    Minecraft,
}

pub fn secret_key(uuid: &str, kind: SecretKind) -> String {
    match kind {
        SecretKind::Refresh => format!("{uuid}/refresh"),
        SecretKind::Minecraft => format!("{uuid}/minecraft"),
    }
}

/// Account metadata as written to `accounts.json` (no tokens).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAccount {
    pub uuid: String,
    pub name: String,
    pub xuid: String,
    pub minecraft_expires_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AccountsFile {
    pub version: u32,
    /// UUID of the default account.
    pub default: Option<String>,
    pub accounts: Vec<StoredAccount>,
}

pub struct AccountStore<S: SecretStore> {
    file: PathBuf,
    secrets: S,
}

impl<S: SecretStore> AccountStore<S> {
    pub fn new(file: PathBuf, secrets: S) -> Self {
        Self { file, secrets }
    }

    /// A missing file is an empty list; an unreadable, malformed or foreign
    /// (other version) file is an error and is left untouched.
    pub fn load(&self) -> Result<AccountsFile> {
        let raw = match std::fs::read_to_string(&self.file) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(AccountsFile {
                    version: FILE_VERSION,
                    ..AccountsFile::default()
                })
            }
            Err(e) => return Err(e.into()),
        };
        let file: AccountsFile = serde_json::from_str(&raw)?;
        if file.version != FILE_VERSION {
            let msg = format!("unsupported accounts file version {}", file.version);
            return Err(<serde_json::Error as serde::de::Error>::custom(msg).into());
        }
        Ok(file)
    }

    /// Temp file next to the target, then rename over it.
    fn write(&self, file: &AccountsFile) -> Result<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut tmp = self.file.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::write(&tmp, serde_json::to_vec_pretty(file)?)?;
        std::fs::rename(&tmp, &self.file)?;
        Ok(())
    }

    /// Upsert by UUID. The file is parsed first, then the tokens are stored,
    /// then the metadata is written. The first account becomes the default.
    pub fn save_session(&self, s: &LoginSession) -> Result<StoredAccount> {
        let mut file = self.load()?;
        let account = StoredAccount {
            uuid: s.profile.id.clone(),
            name: s.profile.name.clone(),
            xuid: s.xuid.clone(),
            minecraft_expires_at: s.minecraft_expires_at,
        };
        self.secrets.set(
            &secret_key(&account.uuid, SecretKind::Refresh),
            &s.refresh_token,
        )?;
        self.secrets.set(
            &secret_key(&account.uuid, SecretKind::Minecraft),
            &s.minecraft_token,
        )?;
        match file.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
            Some(existing) => *existing = account.clone(),
            None => file.accounts.push(account.clone()),
        }
        if file.default.is_none() {
            file.default = Some(account.uuid.clone());
        }
        self.write(&file)?;
        Ok(account)
    }

    /// Name (case-insensitive) or exact UUID; `None` is the default account.
    pub fn find(&self, selector: Option<&str>) -> Result<StoredAccount> {
        let file = self.load()?;
        pick(&file, selector).cloned()
    }

    pub fn set_default(&self, selector: &str) -> Result<StoredAccount> {
        let mut file = self.load()?;
        let account = pick(&file, Some(selector))?.clone();
        file.default = Some(account.uuid.clone());
        self.write(&file)?;
        Ok(account)
    }

    /// Deletes both tokens and the entry. If it was the default, the first
    /// remaining account becomes the default, otherwise there is none.
    pub fn remove(&self, selector: &str) -> Result<StoredAccount> {
        let mut file = self.load()?;
        let account = pick(&file, Some(selector))?.clone();
        self.secrets
            .delete(&secret_key(&account.uuid, SecretKind::Refresh))?;
        self.secrets
            .delete(&secret_key(&account.uuid, SecretKind::Minecraft))?;
        file.accounts.retain(|a| a.uuid != account.uuid);
        if file.default.as_deref() == Some(account.uuid.as_str()) {
            file.default = file.accounts.first().map(|a| a.uuid.clone());
        }
        self.write(&file)?;
        Ok(account)
    }

    /// A missing token means the login has to be redone.
    pub fn secret(&self, account: &StoredAccount, kind: SecretKind) -> Result<Secret> {
        self.secrets
            .get(&secret_key(&account.uuid, kind))?
            .ok_or_else(|| {
                Error::Auth(AuthError::LoginExpired {
                    name: account.name.clone(),
                })
            })
    }
}

fn pick<'a>(file: &'a AccountsFile, selector: Option<&str>) -> Result<&'a StoredAccount> {
    match selector {
        None => file
            .default
            .as_deref()
            .and_then(|uuid| file.accounts.iter().find(|a| a.uuid == uuid))
            .ok_or(Error::Auth(AuthError::NoAccount)),
        Some(sel) => {
            let lower = sel.to_lowercase();
            file.accounts
                .iter()
                .find(|a| a.uuid == sel || a.name.to_lowercase() == lower)
                .ok_or_else(|| {
                    Error::Auth(AuthError::AccountNotFound {
                        selector: sel.to_string(),
                    })
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::minecraft::Profile;
    use crate::auth::AuthError;
    use crate::Error;

    const UUID_A: &str = "0123456789abcdef0123456789abcdef";
    const UUID_B: &str = "fedcba9876543210fedcba9876543210";

    fn session(name: &str, uuid: &str, mc: &str, rt: &str) -> LoginSession {
        LoginSession {
            profile: Profile {
                id: uuid.into(),
                name: name.into(),
            },
            xuid: "2535".into(),
            minecraft_token: Secret::new(mc),
            minecraft_expires_at: 1_000,
            refresh_token: Secret::new(rt),
        }
    }

    fn fresh() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        AccountStore<MemorySecretStore>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join("accounts.json");
        let store = AccountStore::new(file.clone(), MemorySecretStore::default());
        (dir, file, store)
    }

    fn has_secret(store: &AccountStore<MemorySecretStore>, uuid: &str, kind: SecretKind) -> bool {
        store
            .secrets
            .get(&secret_key(uuid, kind))
            .unwrap()
            .is_some()
    }

    #[test]
    fn first_login_becomes_default_and_secrets_stay_out_of_the_file() {
        let (_d, file, store) = fresh();
        let acc = store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        let raw = std::fs::read_to_string(&file).unwrap();
        assert!(!raw.contains("mc-a") && !raw.contains("rt-a"));
        assert!(!file.with_extension("json.tmp").exists());
        assert_eq!(store.find(None).unwrap().name, "Steve");
        assert_eq!(acc.minecraft_expires_at, 1_000);
        assert_eq!(
            store.secret(&acc, SecretKind::Refresh).unwrap().expose(),
            "rt-a"
        );
        assert_eq!(
            store.secret(&acc, SecretKind::Minecraft).unwrap().expose(),
            "mc-a"
        );
    }

    #[test]
    fn relogin_updates_name_and_tokens_without_duplicate() {
        let (_d, _f, store) = fresh();
        store
            .save_session(&session("Steve", UUID_A, "mc-1", "rt-1"))
            .unwrap();
        store
            .save_session(&session("Other", UUID_B, "mc-b", "rt-b"))
            .unwrap();
        let acc = store
            .save_session(&session("Steve2", UUID_A, "mc-2", "rt-2"))
            .unwrap();
        let file = store.load().unwrap();
        assert_eq!(file.accounts.len(), 2);
        assert_eq!(file.default.as_deref(), Some(UUID_A));
        assert_eq!(store.find(Some(UUID_A)).unwrap().name, "Steve2");
        assert_eq!(
            store.secret(&acc, SecretKind::Refresh).unwrap().expose(),
            "rt-2"
        );
        assert_eq!(
            store.secret(&acc, SecretKind::Minecraft).unwrap().expose(),
            "mc-2"
        );
    }

    #[test]
    fn selectors_are_case_insensitive_and_default_moves_on_remove() {
        let (_d, _f, store) = fresh();
        store
            .save_session(&session("Dev", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        store
            .save_session(&session("Alex", UUID_B, "mc-b", "rt-b"))
            .unwrap();
        assert_eq!(store.find(Some("dev")).unwrap().uuid, UUID_A);
        assert_eq!(store.find(Some("ALEX")).unwrap().uuid, UUID_B);
        assert_eq!(store.find(Some(UUID_B)).unwrap().name, "Alex");
        assert_eq!(store.find(None).unwrap().name, "Dev");

        store.set_default("alex").unwrap();
        assert_eq!(store.find(None).unwrap().name, "Alex");
        store.set_default("dev").unwrap();

        let removed = store.remove("dev").unwrap();
        assert_eq!(removed.uuid, UUID_A);
        assert_eq!(store.find(None).unwrap().name, "Alex");
        assert!(!has_secret(&store, UUID_A, SecretKind::Refresh));
        assert!(!has_secret(&store, UUID_A, SecretKind::Minecraft));
        assert!(has_secret(&store, UUID_B, SecretKind::Refresh));

        store.remove("alex").unwrap();
        assert_eq!(store.load().unwrap().default, None);
        assert!(!has_secret(&store, UUID_B, SecretKind::Refresh));
        assert!(!has_secret(&store, UUID_B, SecretKind::Minecraft));
        assert!(matches!(
            store.find(None),
            Err(Error::Auth(AuthError::NoAccount))
        ));
    }

    #[test]
    fn unknown_selector_is_a_clear_error() {
        let (_d, _f, store) = fresh();
        store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        let e = store.find(Some("Nobody")).unwrap_err();
        assert!(matches!(e, Error::Auth(AuthError::AccountNotFound { .. })));
        assert!(e.to_string().contains("\"Nobody\""));
        assert!(store.remove("Nobody").is_err());
        assert!(store.set_default("Nobody").is_err());
    }

    #[test]
    fn missing_secret_means_login_expired() {
        let (_d, _f, store) = fresh();
        let acc = store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        store
            .secrets
            .delete(&secret_key(UUID_A, SecretKind::Refresh))
            .unwrap();
        let e = store.secret(&acc, SecretKind::Refresh).unwrap_err();
        assert!(matches!(e, Error::Auth(AuthError::LoginExpired { ref name }) if name == "Steve"));
    }

    #[test]
    fn missing_file_loads_as_empty_and_find_reports_no_account() {
        let (_d, _f, store) = fresh();
        let f = store.load().unwrap();
        assert!(f.accounts.is_empty() && f.default.is_none());
        assert!(matches!(
            store.find(None),
            Err(Error::Auth(AuthError::NoAccount))
        ));
    }

    #[test]
    fn corrupt_file_is_reported_not_replaced() {
        let (_d, file, store) = fresh();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "{not json").unwrap();
        assert!(matches!(store.load(), Err(Error::Json(_))));
        assert!(store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{not json");
        assert!(!has_secret(&store, UUID_A, SecretKind::Refresh));
        assert!(!has_secret(&store, UUID_A, SecretKind::Minecraft));
    }

    #[test]
    fn foreign_version_is_rejected_not_replaced() {
        let (_d, file, store) = fresh();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let foreign = r#"{"version":2,"default":null,"accounts":[]}"#;
        std::fs::write(&file, foreign).unwrap();
        assert!(matches!(store.load(), Err(Error::Json(_))));
        assert!(store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), foreign);
        assert!(!has_secret(&store, UUID_A, SecretKind::Refresh));
    }

    #[test]
    fn secret_keys_use_uuid_and_kind() {
        assert_eq!(secret_key("abc", SecretKind::Refresh), "abc/refresh");
        assert_eq!(secret_key("abc", SecretKind::Minecraft), "abc/minecraft");
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "touches the real Windows Credential Manager"]
    fn keyring_round_trip() {
        let store = KeyringSecretStore::new().unwrap();
        let key = format!(
            "hengst-test/{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let value = Secret::new("x".repeat(2000));
        store.set(&key, &value).unwrap();
        assert_eq!(store.get(&key).unwrap().unwrap(), value);
        store.delete(&key).unwrap();
        assert!(store.get(&key).unwrap().is_none());
        store.delete(&key).unwrap();
    }
}
