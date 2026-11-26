use crate::dto::play::GameStateDto;
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
    DataChanged {
        scope: DataScope,
    },
    Notice {
        kind: NoticeKind,
    },
    /// Every change of the game's state (`play`), in order.
    GameState {
        state: GameStateDto,
    },
    /// "Quit Hengst Client" in the tray while a game runs: the window is shown, and the web view
    /// asks whether to stop the game and quit (DESIGN.md 11.11). Nothing else happens until then.
    QuitRequested,
    /// "Stop game" in the tray reached no window of run `run_id`: nothing was asked to close (as
    /// `StopResult::NoWindow` of `game_stop`). The window is shown so the start page can say so.
    #[serde(rename_all = "camelCase")]
    StopNoWindow {
        run_id: crate::dto::play::RunId,
    },
}

pub type EventSink<T> = Arc<dyn Fn(T) + Send + Sync>;

/// The web view's side of the hub: its channel, and a quit request that waits for a page.
#[derive(Default)]
struct Page {
    sink: Option<EventSink<AppEvent>>,
    quit_pending: bool,
}

/// Delivers events to the one subscriber (the web view's channel) and to the backend's observers
/// (the tray). Clones share both.
#[derive(Clone, Default)]
pub struct EventHub {
    page: Arc<Mutex<Page>>,
    observers: Arc<Mutex<Vec<EventSink<AppEvent>>>>,
}

impl EventHub {
    fn page(&self) -> std::sync::MutexGuard<'_, Page> {
        self.page.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Replaces an earlier subscriber. Observers stay. A quit request that waited for a page goes
    /// to this one.
    pub fn subscribe(&self, sink: EventSink<AppEvent>) {
        let pending = {
            let mut page = self.page();
            page.sink = Some(sink.clone());
            std::mem::take(&mut page.quit_pending)
        };
        if pending {
            sink(AppEvent::QuitRequested);
        }
    }

    /// The page is gone (it starts loading again): until it subscribes, nothing listens.
    pub fn unsubscribe(&self) {
        self.page().sink = None;
    }

    /// `AppEvent::QuitRequested` to the page, or, while no page listens, to the next one that
    /// subscribes (once). The tray's quit must not get lost in a reload.
    pub fn request_quit(&self) {
        let sink = {
            let mut page = self.page();
            if page.sink.is_none() {
                page.quit_pending = true;
            }
            page.sink.clone()
        };
        if let Some(sink) = sink {
            sink(AppEvent::QuitRequested);
        }
    }

    /// Adds a backend listener that gets every event too, for as long as the hub lives. It is
    /// called on the emitting thread, possibly while the emitter holds a lock (the game emits
    /// under its state lock): it must only hand the event on (a channel), never wait, and never
    /// call back into whatever emitted it.
    pub fn observe(&self, observer: EventSink<AppEvent>) {
        self.observers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(observer);
    }

    /// Without a subscriber the web view's copy is dropped; observers get it anyway.
    pub fn emit(&self, e: AppEvent) {
        let sink = self.page().sink.clone();
        let observers = self
            .observers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        for observer in &observers {
            observer(e.clone());
        }
        if let Some(sink) = sink {
            sink(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorder() -> (EventSink<AppEvent>, Arc<Mutex<Vec<AppEvent>>>) {
        let seen: Arc<Mutex<Vec<AppEvent>>> = Arc::default();
        let kept = seen.clone();
        (Arc::new(move |e| kept.lock().unwrap().push(e)), seen)
    }

    #[test]
    fn a_new_subscriber_replaces_the_old_one_but_never_an_observer() {
        let hub = EventHub::default();
        let (first, first_seen) = recorder();
        let (second, second_seen) = recorder();
        let (observer, observed) = recorder();
        hub.observe(observer);
        hub.subscribe(first);
        hub.subscribe(second);
        let e = AppEvent::QuitRequested;
        hub.emit(e.clone());
        assert!(first_seen.lock().unwrap().is_empty());
        assert_eq!(*second_seen.lock().unwrap(), std::slice::from_ref(&e));
        assert_eq!(*observed.lock().unwrap(), [e]);
    }

    /// The tray's quit request while no page listens (it reloads, or it has not subscribed yet)
    /// waits for the next subscriber and reaches it exactly once.
    #[test]
    fn a_quit_request_without_a_page_reaches_the_next_subscriber_once() {
        let hub = EventHub::default();
        hub.request_quit();
        let (first, first_seen) = recorder();
        hub.subscribe(first);
        assert_eq!(*first_seen.lock().unwrap(), [AppEvent::QuitRequested]);
        let (second, second_seen) = recorder();
        hub.subscribe(second);
        assert!(second_seen.lock().unwrap().is_empty());

        // A page that starts loading has no channel any more: its request waits too.
        hub.unsubscribe();
        hub.request_quit();
        let (third, third_seen) = recorder();
        hub.subscribe(third);
        assert_eq!(*third_seen.lock().unwrap(), [AppEvent::QuitRequested]);
    }

    #[test]
    fn a_quit_request_with_a_page_goes_out_at_once_and_not_again() {
        let hub = EventHub::default();
        let (first, first_seen) = recorder();
        hub.subscribe(first);
        hub.request_quit();
        assert_eq!(*first_seen.lock().unwrap(), [AppEvent::QuitRequested]);
        let (second, second_seen) = recorder();
        hub.subscribe(second);
        assert!(second_seen.lock().unwrap().is_empty());
    }

    #[test]
    fn observers_get_events_without_a_subscriber() {
        let hub = EventHub::default();
        let (observer, observed) = recorder();
        hub.observe(observer);
        hub.emit(AppEvent::QuitRequested);
        assert_eq!(observed.lock().unwrap().len(), 1);
    }
}
