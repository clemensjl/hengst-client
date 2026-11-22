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

/// A boxed store is a store: the desktop app picks the real or the in-memory one at start-up.
impl<S: SecretStore + ?Sized> SecretStore for Box<S> {
    fn get(&self, key: &str) -> Result<Option<Secret>> {
        (**self).get(key)
    }

    fn set(&self, key: &str, value: &Secret) -> Result<()> {
        (**self).set(key, value)
    }

    fn delete(&self, key: &str) -> Result<()> {
        (**self).delete(key)
    }
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
    /// The saved login stopped working (or could not be cleaned up completely): the user has to
    /// sign in again. Not written while false, so files stay as older versions know them.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sign_in_required: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AccountsFile {
    pub version: u32,
    /// UUID of the default account.
    pub default: Option<String>,
    pub accounts: Vec<StoredAccount>,
}

/// Reads the account metadata file without touching any secret store, e.g.
/// for listing. A missing file is an empty list; an unreadable, malformed or
/// foreign (other version) file is an error.
pub fn load_accounts_file(path: &std::path::Path) -> Result<AccountsFile> {
    let raw = match std::fs::read_to_string(path) {
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
        load_accounts_file(&self.file)
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
            sign_in_required: false,
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

    /// Records that this account has to sign in again (see `StoredAccount::sign_in_required`).
    /// Touches the file only; a new `save_session` clears the flag.
    pub fn mark_sign_in_required(&self, selector: &str) -> Result<StoredAccount> {
        let mut file = self.load()?;
        let uuid = pick(&file, Some(selector))?.uuid.clone();
        let account = file
            .accounts
            .iter_mut()
            .find(|a| a.uuid == uuid)
            .expect("picked from this list");
        account.sign_in_required = true;
        let marked = account.clone();
        self.write(&file)?;
        Ok(marked)
    }

    /// Deletes both tokens and the entry. If it was the default, the first remaining account
    /// becomes the default, otherwise there is none.
    ///
    /// If the first delete fails nothing has changed and that error is returned. Any later
    /// failure (the second delete or the write) leaves an entry whose tokens may be partly gone:
    /// it is marked `sign_in_required` (one best-effort write of the file as it was, the entry
    /// flagged) and the original error is returned. A secret that is already missing counts as
    /// deleted, so calling `remove` again completes the job.
    pub fn remove(&self, selector: &str) -> Result<StoredAccount> {
        let before = self.load()?;
        let account = pick(&before, Some(selector))?.clone();
        self.secrets
            .delete(&secret_key(&account.uuid, SecretKind::Refresh))?;
        let finish = || -> Result<()> {
            self.secrets
                .delete(&secret_key(&account.uuid, SecretKind::Minecraft))?;
            let mut file = AccountsFile {
                version: before.version,
                default: before.default.clone(),
                accounts: before.accounts.clone(),
            };
            file.accounts.retain(|a| a.uuid != account.uuid);
            if file.default.as_deref() == Some(account.uuid.as_str()) {
                file.default = file.accounts.first().map(|a| a.uuid.clone());
            }
            self.write(&file)
        };
        match finish() {
            Ok(()) => Ok(account),
            Err(e) => {
                let mut flagged = before;
                if let Some(entry) = flagged.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
                    entry.sign_in_required = true;
                }
                // Best effort: it may fail for the same reason as the write that just failed.
                let _ = self.write(&flagged);
                Err(e)
            }
        }
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

    #[test]
    fn accounts_can_be_listed_without_a_secret_store() {
        let (_d, file, store) = fresh();
        store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        let listed = load_accounts_file(&file).unwrap();
        assert_eq!(listed.accounts.len(), 1);
        assert_eq!(listed.accounts[0].name, "Steve");
        assert_eq!(listed.default.as_deref(), Some(UUID_A));
        let missing = load_accounts_file(&file.with_file_name("none.json")).unwrap();
        assert!(missing.accounts.is_empty());
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
    fn boxed_secret_store_delegates() {
        let boxed: Box<dyn SecretStore> = Box::new(MemorySecretStore::default());
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::new(dir.path().join("accounts.json"), boxed);
        let acc = store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        assert_eq!(
            store.secret(&acc, SecretKind::Minecraft).unwrap().expose(),
            "mc-a"
        );
        store.remove(UUID_A).unwrap();
        assert!(matches!(
            store.secret(&acc, SecretKind::Minecraft),
            Err(Error::Auth(AuthError::LoginExpired { .. }))
        ));
    }

    #[test]
    fn sign_in_required_is_saved_cleared_by_a_new_session_and_absent_from_old_files() {
        let (_d, file, store) = fresh();
        store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        // An account without the flag does not write the key (the file stays as the CLI knows it).
        assert!(!std::fs::read_to_string(&file)
            .unwrap()
            .contains("sign_in_required"));
        assert!(!store.find(Some(UUID_A)).unwrap().sign_in_required);
        store.mark_sign_in_required(UUID_A).unwrap();
        assert!(load_accounts_file(&file).unwrap().accounts[0].sign_in_required);
        // The secrets stay: marking only records that the saved login stopped working.
        assert!(has_secret(&store, UUID_A, SecretKind::Refresh));
        store
            .save_session(&session("Steve", UUID_A, "mc-a2", "rt-a2"))
            .unwrap();
        assert!(!load_accounts_file(&file).unwrap().accounts[0].sign_in_required);
        assert!(matches!(
            store.mark_sign_in_required("nobody"),
            Err(Error::Auth(AuthError::AccountNotFound { .. }))
        ));
    }

    /// Delegates to a memory store; refuses every delete from the n-th call on (1 = the first).
    struct RefusingDeletes {
        inner: MemorySecretStore,
        calls: std::sync::atomic::AtomicUsize,
        refuse_from: std::sync::atomic::AtomicUsize,
    }

    impl RefusingDeletes {
        fn new(refuse_from: usize) -> Self {
            Self {
                inner: MemorySecretStore::default(),
                calls: Default::default(),
                refuse_from: refuse_from.into(),
            }
        }
    }

    impl SecretStore for std::sync::Arc<RefusingDeletes> {
        fn get(&self, key: &str) -> Result<Option<Secret>> {
            self.inner.get(key)
        }

        fn set(&self, key: &str, value: &Secret) -> Result<()> {
            self.inner.set(key, value)
        }

        fn delete(&self, key: &str) -> Result<()> {
            use std::sync::atomic::Ordering::SeqCst;
            let n = self.calls.fetch_add(1, SeqCst) + 1;
            let from = self.refuse_from.load(SeqCst);
            if from != 0 && n >= from {
                return Err(Error::CredentialStore("refused".into()));
            }
            self.inner.delete(key)
        }
    }

    fn refusing(
        refuse_from: usize,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::sync::Arc<RefusingDeletes>,
        AccountStore<std::sync::Arc<RefusingDeletes>>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("accounts.json");
        let secrets = std::sync::Arc::new(RefusingDeletes::new(refuse_from));
        let store = AccountStore::new(file.clone(), secrets.clone());
        store
            .save_session(&session("Steve", UUID_A, "mc-a", "rt-a"))
            .unwrap();
        store
            .save_session(&session("Alex", UUID_B, "mc-b", "rt-b"))
            .unwrap();
        (dir, file, secrets, store)
    }

    fn has(secrets: &RefusingDeletes, uuid: &str, kind: SecretKind) -> bool {
        secrets
            .inner
            .get(&secret_key(uuid, kind))
            .unwrap()
            .is_some()
    }

    #[test]
    fn remove_whose_first_delete_fails_changes_nothing() {
        let (_d, file, secrets, store) = refusing(1);
        let e = store.remove(UUID_B).unwrap_err();
        assert!(matches!(e, Error::CredentialStore(_)), "{e}");
        let listed = load_accounts_file(&file).unwrap();
        assert_eq!(listed.accounts.len(), 2);
        assert!(listed.accounts.iter().all(|a| !a.sign_in_required));
        assert!(has(&secrets, UUID_B, SecretKind::Refresh));
        assert!(has(&secrets, UUID_B, SecretKind::Minecraft));
    }

    #[test]
    fn remove_whose_second_delete_fails_flags_the_entry_and_reports_the_original_error() {
        let (_d, file, secrets, store) = refusing(2);
        let e = store.remove(UUID_B).unwrap_err();
        assert!(matches!(e, Error::CredentialStore(_)), "{e}");
        let listed = load_accounts_file(&file).unwrap();
        assert_eq!(listed.accounts.len(), 2);
        let b = listed.accounts.iter().find(|a| a.uuid == UUID_B).unwrap();
        assert!(b.sign_in_required);
        let a = listed.accounts.iter().find(|a| a.uuid == UUID_A).unwrap();
        assert!(!a.sign_in_required);
        assert!(
            !has(&secrets, UUID_B, SecretKind::Refresh),
            "one secret is gone"
        );
        // A later remove completes: the missing secret counts as deleted.
        secrets
            .refuse_from
            .store(0, std::sync::atomic::Ordering::SeqCst);
        store.remove(UUID_B).unwrap();
        assert_eq!(load_accounts_file(&file).unwrap().accounts.len(), 1);
        assert!(!has(&secrets, UUID_B, SecretKind::Minecraft));
    }

    #[test]
    fn remove_whose_write_fails_after_both_deletes_is_reported_and_completes_later() {
        let (dir, file, secrets, store) = refusing(0);
        // The temp file of the atomic write cannot be created: a folder sits in its place.
        let tmp = dir.path().join("accounts.json.tmp");
        std::fs::create_dir(&tmp).unwrap();
        let e = store.remove(UUID_B).unwrap_err();
        assert!(matches!(e, Error::Io(_)), "the original error: {e}");
        assert!(!has(&secrets, UUID_B, SecretKind::Refresh));
        assert!(!has(&secrets, UUID_B, SecretKind::Minecraft));
        // The entry is still listed (the flag write needs the same temp file and failed too).
        assert_eq!(load_accounts_file(&file).unwrap().accounts.len(), 2);
        std::fs::remove_dir(&tmp).unwrap();
        store.remove(UUID_B).unwrap();
        let listed = load_accounts_file(&file).unwrap();
        assert_eq!(listed.accounts.len(), 1);
        assert_eq!(listed.default.as_deref(), Some(UUID_A));
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
