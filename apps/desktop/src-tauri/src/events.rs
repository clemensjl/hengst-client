use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DataScope {
    Versions,
    Settings,
    Accounts,
    Instances,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NoticeKind {
    /// The background refresh of the version list failed for another reason than being offline.
    VersionsRefreshFailed,
    /// The background refresh of the version list failed because the server was not reachable.
    VersionsRefreshOffline,
}

/// What the backend tells the web view on its own initiative.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum AppEvent {
    DataChanged { scope: DataScope },
    Notice { kind: NoticeKind },
}

pub type EventSink<T> = Arc<dyn Fn(T) + Send + Sync>;

/// Delivers events to the one subscriber (the web view's channel). Clones share the subscriber.
#[derive(Clone, Default)]
pub struct EventHub {
    sink: Arc<Mutex<Option<EventSink<AppEvent>>>>,
}

impl EventHub {
    /// Replaces an earlier subscriber.
    pub fn subscribe(&self, sink: EventSink<AppEvent>) {
        *self.sink.lock().unwrap_or_else(|p| p.into_inner()) = Some(sink);
    }

    /// Without a subscriber the event is dropped.
    pub fn emit(&self, e: AppEvent) {
        let sink = self.sink.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(sink) = sink {
            sink(e);
        }
    }
}
