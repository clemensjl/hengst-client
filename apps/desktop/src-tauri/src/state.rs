use crate::accounts::{Accounts, Operations};
use crate::events::EventHub;
use crate::settings::{load_settings, SettingsHandle};
use hengst_core::auth::unix_now;
use hengst_core::launch::Endpoints;
use hengst_core::manifest::{load_cached, VersionManifest};
use hengst_core::paths::Paths;
use hengst_core::Result;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Moves a folder to the Recycle Bin. The app's one way to remove an instance: it never deletes
/// permanently. Tests replace it.
pub type Trasher = Arc<dyn Fn(&Path) -> Result<()> + Send + Sync>;

pub struct AppState {
    pub paths: Paths,
    pub endpoints: Endpoints,
    pub events: EventHub,
    pub versions: VersionsCache,
    pub settings: SettingsHandle,
    pub accounts: Arc<Accounts>,
    pub operations: Operations,
    /// The one lock of `instances_list` (with its adoption of CLI folders), `instance_create` and
    /// `instance_delete`: a list never adopts a folder that a create has made but not yet filled.
    pub instance_lock: tokio::sync::Mutex<()>,
    /// The instance the app is preparing or running. Always `None` in 4a; 4b sets it. An instance
    /// the CLI started is invisible to the app and not covered.
    pub active_instance: Mutex<Option<String>>,
    pub trasher: Trasher,
}

impl AppState {
    /// Without a secret store and without sign-in: for tests and tools that do not use accounts.
    pub fn new(paths: Paths, endpoints: Endpoints) -> AppState {
        let accounts = Accounts::without_secret_store(
            &paths,
            "no secret store configured".to_string(),
            None,
            Arc::new(|_| Err(())),
        );
        AppState::with_accounts(paths, endpoints, accounts)
    }

    pub fn with_accounts(paths: Paths, endpoints: Endpoints, accounts: Accounts) -> AppState {
        let loaded = load_settings(&paths, unix_now());
        if let Some(reason) = &loaded.blocked {
            eprintln!("settings: {reason}; the defaults are used and nothing is saved until the file can be read");
        }
        AppState {
            settings: SettingsHandle::from_loaded(paths.clone(), loaded),
            paths,
            endpoints,
            events: EventHub::default(),
            versions: VersionsCache::default(),
            accounts: Arc::new(accounts),
            operations: Operations::default(),
            instance_lock: tokio::sync::Mutex::new(()),
            active_instance: Mutex::new(None),
            trasher: Arc::new(crate::recycle::move_to_recycle_bin),
        }
    }
}

#[derive(Default)]
struct Inner {
    memory: Mutex<Option<VersionManifest>>,
    refresh_started: AtomicBool,
}

/// The version list in memory, backed by the saved copy. Clones share the same data, so a
/// background refresh can update what the commands read.
#[derive(Clone, Default)]
pub struct VersionsCache {
    inner: Arc<Inner>,
}

impl VersionsCache {
    /// The list from memory, otherwise the saved copy (which then stays in memory).
    pub async fn get(&self, paths: &Paths) -> Result<Option<VersionManifest>> {
        let held = self
            .inner
            .memory
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if held.is_some() {
            return Ok(held);
        }
        // Read outside the lock; a list stored meanwhile by `put` wins over the saved copy.
        let loaded = load_cached(paths).await?;
        let mut memory = self.inner.memory.lock().unwrap_or_else(|p| p.into_inner());
        if memory.is_none() {
            *memory = loaded;
        }
        Ok(memory.clone())
    }

    pub fn put(&self, m: VersionManifest) {
        *self.inner.memory.lock().unwrap_or_else(|p| p.into_inner()) = Some(m);
    }

    /// `true` exactly once per app run.
    pub fn claim_refresh(&self) -> bool {
        !self.inner.refresh_started.swap(true, Ordering::SeqCst)
    }
}
