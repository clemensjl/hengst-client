use crate::events::EventHub;
use hengst_core::launch::Endpoints;
use hengst_core::manifest::{load_cached, VersionManifest};
use hengst_core::paths::Paths;
use hengst_core::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub paths: Paths,
    pub endpoints: Endpoints,
    pub events: EventHub,
    pub versions: VersionsCache,
}

impl AppState {
    pub fn new(paths: Paths, endpoints: Endpoints) -> AppState {
        AppState {
            paths,
            endpoints,
            events: EventHub::default(),
            versions: VersionsCache::default(),
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
