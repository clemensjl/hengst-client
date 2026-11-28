use crate::dto::error::{ErrorCode, ErrorDto};
use crate::dto::versions::{GameVersionDto, GameVersionKind, GameVersionsDto, VersionFilter};
use crate::error::error_dto;
use crate::events::{AppEvent, DataScope, NoticeKind};
use crate::state::AppState;
use hengst_core::download::http_client;
use hengst_core::manifest::{download_manifest, fetch_manifest, VersionKind, VersionManifest};
use tauri::State;

fn to_dto(m: &VersionManifest, filter: VersionFilter, from_cache: bool) -> GameVersionsDto {
    let versions = m
        .versions
        .iter()
        .filter_map(|v| {
            let (kind, wanted) = match v.kind {
                VersionKind::Release => (GameVersionKind::Release, filter.release),
                VersionKind::Snapshot => (GameVersionKind::Snapshot, filter.snapshot),
                VersionKind::OldBeta => (GameVersionKind::Beta, filter.beta),
                VersionKind::OldAlpha => (GameVersionKind::Alpha, filter.alpha),
                VersionKind::Other => (GameVersionKind::Other, false),
            };
            wanted.then(|| GameVersionDto {
                id: v.id.clone(),
                kind,
                release_time: v.release_time.clone(),
            })
        })
        .collect();
    GameVersionsDto {
        versions,
        latest_release: m.latest.release.clone(),
        latest_snapshot: m.latest.snapshot.clone(),
        from_cache,
    }
}

/// Refreshes the saved list once in the background. A different result is announced with
/// `DataChanged`, a failure with a `Notice` and the saved list stays in use.
fn spawn_refresh(state: &AppState, known: &VersionManifest) {
    let paths = state.paths.clone();
    let url = state.endpoints.manifest.clone();
    let hub = state.events.clone();
    let cache = state.versions.clone();
    let known = (
        known.latest.release.clone(),
        known.latest.snapshot.clone(),
        known.versions.len(),
    );
    tokio::spawn(async move {
        match download_manifest(&http_client(), &paths, &url).await {
            Ok(fresh) => {
                let changed = known
                    != (
                        fresh.latest.release.clone(),
                        fresh.latest.snapshot.clone(),
                        fresh.versions.len(),
                    );
                cache.put(fresh);
                if changed {
                    hub.emit(AppEvent::DataChanged {
                        scope: DataScope::Versions,
                    });
                }
            }
            Err(e) => {
                let dto = error_dto(&e);
                eprintln!("version list refresh failed: {}", dto.detail);
                let kind = match dto.code {
                    ErrorCode::Network => NoticeKind::VersionsRefreshOffline,
                    _ => NoticeKind::VersionsRefreshFailed,
                };
                hub.emit(AppEvent::Notice { kind });
            }
        }
    });
}

pub async fn game_versions_impl(
    state: &AppState,
    filter: VersionFilter,
) -> Result<GameVersionsDto, ErrorDto> {
    if let Some(cached) = state
        .versions
        .get(&state.paths)
        .await
        .map_err(|e| error_dto(&e))?
    {
        if state.versions.claim_refresh() {
            spawn_refresh(state, &cached);
        }
        return Ok(to_dto(&cached, filter, true));
    }
    let fresh = fetch_manifest(&http_client(), &state.paths, &state.endpoints.manifest)
        .await
        .map_err(|e| error_dto(&e))?;
    // The list was just downloaded: no refresh is due this run.
    state.versions.claim_refresh();
    state.versions.put(fresh.clone());
    Ok(to_dto(&fresh, filter, false))
}

#[tauri::command]
pub async fn game_versions(
    state: State<'_, AppState>,
    filter: VersionFilter,
) -> Result<GameVersionsDto, ErrorDto> {
    game_versions_impl(&state, filter).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventHub;
    use hengst_core::launch::Endpoints;
    use hengst_core::paths::Paths;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn manifest(latest: &str, with_older: bool) -> serde_json::Value {
        let mut versions = vec![];
        if latest == "1.21.11" {
            versions.push(json!({"id": "25w45a", "type": "snapshot", "url": "u0", "sha1": "s0", "releaseTime": "2025-11-01T00:00:00+00:00"}));
        }
        versions.push(json!({"id": latest, "type": "release", "url": "u1", "sha1": "s1", "releaseTime": "2025-10-01T00:00:00+00:00"}));
        if with_older {
            versions.push(json!({"id": "b1.7.3", "type": "old_beta", "url": "u2", "sha1": "s2", "releaseTime": "2011-07-01T00:00:00+00:00"}));
            versions.push(json!({"id": "a1.2.6", "type": "old_alpha", "url": "u3", "sha1": "s3", "releaseTime": "2010-12-01T00:00:00+00:00"}));
        }
        let snapshot = if latest == "1.21.11" {
            "25w45a"
        } else {
            latest
        };
        json!({"latest": {"release": latest, "snapshot": snapshot}, "versions": versions})
    }

    fn full() -> serde_json::Value {
        manifest("1.21.11", true)
    }

    fn filter(release: bool, snapshot: bool, beta: bool, alpha: bool) -> VersionFilter {
        VersionFilter {
            release,
            snapshot,
            beta,
            alpha,
        }
    }

    fn only_release() -> VersionFilter {
        filter(true, false, false, false)
    }

    fn ids(r: &GameVersionsDto) -> Vec<&str> {
        r.versions.iter().map(|v| v.id.as_str()).collect()
    }

    type Log = Arc<Mutex<Vec<AppEvent>>>;

    fn state_for(dir: &std::path::Path, manifest_url: String) -> (AppState, Log) {
        let endpoints = Endpoints {
            manifest: manifest_url,
            ..Endpoints::default()
        };
        let state = AppState::new(Paths::new(dir), endpoints);
        let log: Log = Arc::default();
        let sink = log.clone();
        state
            .events
            .subscribe(Arc::new(move |e| sink.lock().unwrap().push(e)));
        (state, log)
    }

    fn write_cache(paths: &Paths, body: &str) {
        std::fs::create_dir_all(paths.manifest_file().parent().unwrap()).unwrap();
        std::fs::write(paths.manifest_file(), body).unwrap();
    }

    async fn serve(body: ResponseTemplate) -> MockServer {
        // A server of its own: a pooled one (`MockServer::start`) is handed to the next test after
        // a reset, and a request still on its way from an earlier test would count there.
        let server = MockServer::builder().start().await;
        Mock::given(method("GET"))
            .respond_with(body)
            .mount(&server)
            .await;
        server
    }

    fn closed_port_url() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/manifest.json", listener.local_addr().unwrap());
        drop(listener);
        url
    }

    /// Waits for a background refresh to show its effect. Generous on purpose: a refused
    /// connection to a closed local port alone takes about 2 s on Windows, and a loaded test run
    /// is slower still. Measured by the clock, not by a number of sleeps (whose length depends on
    /// the system timer resolution).
    async fn wait_until(mut done: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !done() {
            assert!(
                std::time::Instant::now() < deadline,
                "condition not met within 30 s"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn data_changed() -> AppEvent {
        AppEvent::DataChanged {
            scope: DataScope::Versions,
        }
    }

    #[tokio::test]
    async fn filter_selects_kinds_in_manifest_order() {
        let server = serve(ResponseTemplate::new(200).set_body_json(full())).await;
        let dir = tempfile::tempdir().unwrap();
        let (state, _) = state_for(dir.path(), server.uri());
        let r = game_versions_impl(&state, filter(true, false, true, false))
            .await
            .unwrap();
        assert_eq!(ids(&r), ["1.21.11", "b1.7.3"]);
        assert_eq!(r.latest_release, "1.21.11");
        assert_eq!(r.latest_snapshot, "25w45a");
        assert!(!r.from_cache);
        assert_eq!(r.versions[1].kind, GameVersionKind::Beta);
        assert_eq!(r.versions[0].release_time, "2025-10-01T00:00:00+00:00");
        let all = game_versions_impl(&state, filter(true, true, true, true))
            .await
            .unwrap();
        assert_eq!(ids(&all), ["25w45a", "1.21.11", "b1.7.3", "a1.2.6"]);
    }

    #[tokio::test]
    async fn cached_list_answers_first_and_refresh_emits_versions_changed() {
        let server = serve(ResponseTemplate::new(200).set_body_json(full())).await;
        let dir = tempfile::tempdir().unwrap();
        let (state, log) = state_for(dir.path(), server.uri());
        write_cache(&state.paths, &manifest("1.21.10", false).to_string());
        let first = game_versions_impl(&state, only_release()).await.unwrap();
        assert_eq!(ids(&first), ["1.21.10"]);
        assert!(first.from_cache);
        wait_until(|| log.lock().unwrap().contains(&data_changed())).await;
        let second = game_versions_impl(&state, only_release()).await.unwrap();
        assert_eq!(ids(&second)[0], "1.21.11");
    }

    #[tokio::test]
    async fn unreachable_server_without_cache_is_a_network_error() {
        let dir = tempfile::tempdir().unwrap();
        let (state, _) = state_for(dir.path(), closed_port_url());
        let dto = game_versions_impl(&state, only_release())
            .await
            .unwrap_err();
        assert_eq!(dto.code, ErrorCode::Network);
    }

    #[test]
    fn hub_delivers_to_latest_subscriber_only() {
        let hub = EventHub::default();
        hub.emit(AppEvent::DataChanged {
            scope: DataScope::Settings,
        });
        let (first, second): (Log, Log) = (Arc::default(), Arc::default());
        let sink = first.clone();
        hub.subscribe(Arc::new(move |e| sink.lock().unwrap().push(e)));
        let sink = second.clone();
        hub.subscribe(Arc::new(move |e| sink.lock().unwrap().push(e)));
        let event = AppEvent::DataChanged {
            scope: DataScope::Accounts,
        };
        hub.emit(event.clone());
        assert!(first.lock().unwrap().is_empty());
        assert_eq!(*second.lock().unwrap(), [event]);
    }

    #[test]
    fn app_info_answers_over_ipc() {
        use tauri::ipc::{CallbackFn, InvokeBody};
        use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
        use tauri::webview::InvokeRequest;
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Paths::new(dir.path()), Endpoints::default());
        let app = mock_builder()
            .manage(state)
            .invoke_handler(with_commands!(handler))
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let response = get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: "app_info".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .unwrap();
        let json: serde_json::Value = response.deserialize().unwrap();
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(json["devBuild"], crate::dev_build());
        assert_eq!(json["dataDir"], dir.path().display().to_string());
    }

    #[test]
    fn dev_build_is_debug_or_dev_offline_or_e2e() {
        assert_eq!(
            crate::dev_build(),
            cfg!(debug_assertions) || cfg!(feature = "dev-offline") || cfg!(feature = "e2e")
        );
    }

    #[tokio::test]
    async fn failed_background_refresh_is_logged_and_surfaced_as_a_notice() {
        let server = serve(ResponseTemplate::new(200).set_body_string("{")).await;
        let dir = tempfile::tempdir().unwrap();
        let (state, log) = state_for(dir.path(), server.uri());
        write_cache(&state.paths, &manifest("1.21.10", false).to_string());
        let first = game_versions_impl(&state, only_release()).await.unwrap();
        assert_eq!(ids(&first), ["1.21.10"]);
        let failed = AppEvent::Notice {
            kind: NoticeKind::VersionsRefreshFailed,
        };
        wait_until(|| log.lock().unwrap().contains(&failed)).await;
        let again = game_versions_impl(&state, only_release()).await.unwrap();
        assert_eq!(ids(&again), ["1.21.10"]);
        assert!(log.lock().unwrap().iter().all(|e| *e != data_changed()));
    }

    #[tokio::test]
    async fn refresh_failing_because_the_server_is_unreachable_is_marked_offline() {
        let dir = tempfile::tempdir().unwrap();
        let (state, log) = state_for(dir.path(), closed_port_url());
        write_cache(&state.paths, &manifest("1.21.10", false).to_string());
        let first = game_versions_impl(&state, only_release()).await.unwrap();
        assert_eq!(ids(&first), ["1.21.10"]);
        let offline = AppEvent::Notice {
            kind: NoticeKind::VersionsRefreshOffline,
        };
        wait_until(|| log.lock().unwrap().contains(&offline)).await;
        assert!(log.lock().unwrap().iter().all(|e| !matches!(
            e,
            AppEvent::Notice {
                kind: NoticeKind::VersionsRefreshFailed
            }
        )));
    }

    #[tokio::test]
    async fn refresh_runs_once_per_app_run() {
        let server = serve(ResponseTemplate::new(200).set_body_json(full())).await;
        let dir = tempfile::tempdir().unwrap();
        let (state, log) = state_for(dir.path(), server.uri());
        write_cache(&state.paths, &manifest("1.21.10", false).to_string());
        for _ in 0..3 {
            game_versions_impl(&state, only_release()).await.unwrap();
        }
        wait_until(|| !log.lock().unwrap().is_empty()).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn offline_with_a_damaged_saved_list_is_a_network_error_with_retry() {
        let dir = tempfile::tempdir().unwrap();
        let (state, _) = state_for(dir.path(), closed_port_url());
        write_cache(&state.paths, "{");
        let dto = game_versions_impl(&state, only_release())
            .await
            .unwrap_err();
        assert_eq!(dto.code, ErrorCode::Network);
        assert_eq!(dto.action, Some(crate::dto::error::ErrorAction::Retry));
    }
}
