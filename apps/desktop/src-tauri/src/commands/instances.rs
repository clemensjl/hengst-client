use crate::accounts::internal;
use crate::commands::versions::game_versions_impl;
use crate::dto::error::{ErrorCode, ErrorDto};
use crate::dto::instances::{
    CreateInstanceRequest, InstanceDto, InstanceLoaderDto, InstanceStatus, LoaderInfoDto,
    LoaderKindDto, LoaderVersionDto, LoaderVersionsDto, LOADER_ORDER,
};
use crate::dto::versions::VersionFilter;
use crate::error::{error_dto, redact};
use crate::events::{AppEvent, DataScope};
use crate::state::AppState;
use hengst_core::auth::unix_now;
use hengst_core::download::http_client;
use hengst_core::instances::{self, Instance, InstanceLoader, Listed, NewInstance};
use hengst_core::loaders::{list_versions, pick_version, LoaderKind};
use hengst_core::manifest::VersionManifest;
use hengst_core::Error;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use tauri::State;

fn join_error(_: tauri::Error) -> ErrorDto {
    internal("a background task failed")
}

fn changed(state: &AppState, scope: DataScope) {
    state.events.emit(AppEvent::DataChanged { scope });
}

fn busy() -> ErrorDto {
    ErrorDto {
        code: ErrorCode::Busy,
        params: BTreeMap::new(),
        action: None,
        detail: "another instance is being created or deleted".to_string(),
    }
}

fn with_id(code: ErrorCode, id: &str, detail: String) -> ErrorDto {
    ErrorDto {
        code,
        params: BTreeMap::from([("id".to_string(), id.to_string())]),
        action: None,
        detail: redact(&detail),
    }
}

/// The error for an instance id the web view sent. An id that is not even a folder name names no
/// instance, so it is "not found", not "the downloaded data was rejected".
fn id_error(e: &Error, id: &str) -> ErrorDto {
    match e {
        Error::UnsafePath(_) => error_dto(&Error::InstanceNotFound { id: id.to_string() }),
        other => error_dto(other),
    }
}

fn loader_dto(l: &InstanceLoader) -> InstanceLoaderDto {
    InstanceLoaderDto {
        kind: l.kind.into(),
        display_name: l.kind.display_name().to_string(),
        version: l.version.clone(),
    }
}

fn ready_dto(i: &Instance, selected: Option<&str>) -> InstanceDto {
    let c = &i.config;
    InstanceDto {
        id: i.id.clone(),
        name: c.name.clone(),
        game: c.game.clone(),
        loader: c.loader.as_ref().map(loader_dto),
        memory_mb: c.memory_mb,
        last_played: c.last_played,
        status: InstanceStatus::Ready,
        reason: None,
        selected: selected == Some(i.id.as_str()),
    }
}

fn listed_dto(l: &Listed, selected: Option<&str>) -> InstanceDto {
    let (id, status, reason) = match l {
        Listed::Ready(i) => return ready_dto(i, selected),
        Listed::TooNew { id, .. } => (id, InstanceStatus::TooNew, None),
        Listed::Broken { id, reason } => (id, InstanceStatus::Broken, Some(reason.clone())),
    };
    InstanceDto {
        id: id.clone(),
        name: id.clone(),
        game: String::new(),
        loader: None,
        memory_mb: None,
        last_played: None,
        status,
        reason,
        selected: selected == Some(id.as_str()),
    }
}

/// The version list: the saved or loaded copy, else one download (which is then saved).
async fn manifest(state: &AppState) -> Result<VersionManifest, ErrorDto> {
    if let Some(m) = state
        .versions
        .get(&state.paths)
        .await
        .map_err(|e| error_dto(&e))?
    {
        return Ok(m);
    }
    let all = VersionFilter {
        release: true,
        snapshot: true,
        beta: true,
        alpha: true,
    };
    game_versions_impl(state, all).await?;
    state
        .versions
        .get(&state.paths)
        .await
        .map_err(|e| error_dto(&e))?
        .ok_or_else(|| internal("the version list is not available"))
}

async fn known_game(state: &AppState, game: &str) -> Result<(), ErrorDto> {
    let m = manifest(state).await?;
    if m.versions.iter().any(|v| v.id == game) {
        return Ok(());
    }
    Err(error_dto(&Error::UnknownVersion(game.to_string())))
}

pub fn loader_kinds_impl() -> Vec<LoaderInfoDto> {
    LOADER_ORDER
        .iter()
        .map(|&kind| LoaderInfoDto {
            kind,
            display_name: LoaderKind::from(kind).display_name().to_string(),
        })
        .collect()
}

/// The builds of a loader for a game version the version list knows. One request to the loader
/// service.
pub async fn loader_versions_impl(
    state: &AppState,
    kind: LoaderKindDto,
    game: &str,
) -> Result<LoaderVersionsDto, ErrorDto> {
    known_game(state, game).await?;
    let kind = LoaderKind::from(kind);
    let list = list_versions(&http_client(), &state.endpoints.loaders, kind, game)
        .await
        .map_err(|e| error_dto(&e))?;
    let recommended = pick_version(&list, None, kind, game)
        .map_err(|e| error_dto(&e))?
        .id
        .clone();
    Ok(LoaderVersionsDto {
        versions: list
            .iter()
            .map(|v| LoaderVersionDto {
                id: v.id.clone(),
                stable: v.stable,
            })
            .collect(),
        recommended,
    })
}

/// The instances, newest played first. Folders the CLI made are adopted first when the version
/// list is saved (without it nothing is adopted: whether a name is a game version is unknown). It
/// waits for a running create or delete, so it never adopts a folder that is half made. A failing
/// adoption is logged and does not fail the list.
pub async fn instances_list_impl(state: &AppState) -> Result<Vec<InstanceDto>, ErrorDto> {
    let _guard = state.instance_lock.lock().await;
    let manifest = match state.versions.get(&state.paths).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "instances: the saved version list is unreadable, no folder is adopted: {}",
                error_dto(&e).detail
            );
            None
        }
    };
    let selected = state.settings.get().selected_instance;
    let paths = state.paths.clone();
    let done = tauri::async_runtime::spawn_blocking(move || {
        let adopted = manifest.map(|m| {
            let games: HashSet<&str> = m.versions.iter().map(|v| v.id.as_str()).collect();
            instances::adopt(&paths, |g| games.contains(g), unix_now())
        });
        (adopted, instances::list(&paths))
    })
    .await;
    // Adoption writes files, also when the task fails afterwards.
    let may_have_changed = match &done {
        Err(_) | Ok((Some(Err(_)), _)) => true,
        Ok((Some(Ok(adopted)), _)) => !adopted.is_empty(),
        Ok((None, _)) => false,
    };
    if may_have_changed {
        changed(state, DataScope::Instances);
    }
    let (adopted, listing) = done.map_err(join_error)?;
    if let Some(Err(e)) = adopted {
        eprintln!(
            "instances: adopting folders failed: {}",
            error_dto(&e).detail
        );
    }
    let listing = listing.map_err(|e| error_dto(&e))?;
    if listing.incomplete {
        eprintln!("instances: some entries of the instances folder could not be read");
    }
    Ok(listing
        .items
        .iter()
        .map(|l| listed_dto(l, selected.as_deref()))
        .collect())
}

/// Makes an instance and selects it. The game version must be in the version list, the loader
/// version (the recommended one when none is named) in the loader service's list: that is one
/// request to the service, and it fails offline. No other network call.
pub async fn instance_create_impl(
    state: &AppState,
    req: CreateInstanceRequest,
) -> Result<InstanceDto, ErrorDto> {
    let Ok(_guard) = state.instance_lock.try_lock() else {
        return Err(busy());
    };
    let name = instances::validate_name(&req.name).map_err(|e| error_dto(&e))?;
    known_game(state, &req.game).await?;
    let loader = match &req.loader {
        None => None,
        Some(l) => {
            let kind = LoaderKind::from(l.kind);
            let list = list_versions(&http_client(), &state.endpoints.loaders, kind, &req.game)
                .await
                .map_err(|e| error_dto(&e))?;
            let picked = pick_version(&list, l.version.as_deref(), kind, &req.game)
                .map_err(|e| error_dto(&e))?;
            Some(InstanceLoader {
                kind,
                version: Some(picked.id.clone()),
            })
        }
    };
    let new = NewInstance {
        name,
        game: req.game.clone(),
        loader,
    };
    let paths = state.paths.clone();
    let made =
        tauri::async_runtime::spawn_blocking(move || instances::create(&paths, &new, unix_now()))
            .await;
    // A refused create changed nothing; a failed task may have made the folder.
    if !matches!(made, Ok(Err(_))) {
        changed(state, DataScope::Instances);
    }
    let made = made.map_err(join_error)?.map_err(|e| error_dto(&e))?;

    let (settings, id) = (state.settings.clone(), made.id.clone());
    let selected = tauri::async_runtime::spawn_blocking(move || {
        settings.update(|s| {
            s.selected_instance = Some(id);
            Ok(())
        })
    })
    .await;
    let selected = match selected {
        Ok(Ok(_)) => {
            changed(state, DataScope::Settings);
            true
        }
        // The instance exists; only the selection could not be saved.
        Ok(Err(e)) => {
            eprintln!(
                "instances: the new instance could not be selected: {}",
                error_dto(&e).detail
            );
            false
        }
        Err(_) => false,
    };
    Ok(ready_dto(&made, selected.then_some(made.id.as_str())))
}

/// Selects an instance that can be read. One that cannot is refused with its own code.
pub async fn instance_select_impl(state: &AppState, id: String) -> Result<(), ErrorDto> {
    let (paths, wanted) = (state.paths.clone(), id.clone());
    tauri::async_runtime::spawn_blocking(move || instances::load(&paths, &wanted))
        .await
        .map_err(join_error)?
        .map_err(|e| id_error(&e, &id))?;
    let settings = state.settings.clone();
    tauri::async_runtime::spawn_blocking(move || {
        settings.update(|s| {
            s.selected_instance = Some(id);
            Ok(())
        })
    })
    .await
    .map_err(join_error)?
    .map_err(|e| error_dto(&e))?;
    changed(state, DataScope::Settings);
    Ok(())
}

/// Moves an instance's folder to the Recycle Bin (`AppState::trasher`). Never deletes for good.
///
/// - The instance the app is preparing or running is refused (`InstanceRunning`). An instance the
///   CLI started is not known to the app and not covered.
/// - If the trasher fails with the folder untouched, the error is `RecycleBinFailed`, nothing
///   changed and no event is sent. Only the trasher's own failure gets that code.
/// - If it fails after the folder is gone, the instance is gone: success. If it fails halfway,
///   the error is `RecycleBinPartial`.
pub async fn instance_delete_impl(state: &AppState, id: String) -> Result<(), ErrorDto> {
    let Ok(_guard) = state.instance_lock.try_lock() else {
        return Err(busy());
    };
    let active = state
        .active_instance
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if active.as_deref() == Some(id.as_str()) {
        return Err(with_id(
            ErrorCode::InstanceRunning,
            &id,
            format!("instance {id} is being prepared or runs"),
        ));
    }
    let (paths, trasher, wanted) = (state.paths.clone(), state.trasher.clone(), id.clone());
    let trash_failure: Arc<Mutex<Option<String>>> = Arc::default();
    let seen = trash_failure.clone();
    let done = tauri::async_runtime::spawn_blocking(move || {
        instances::delete(&paths, &wanted, &|folder| {
            trasher(folder).inspect_err(|e| {
                *seen.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.to_string());
            })
        })
    })
    .await;
    let trash_failure = trash_failure
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let result = done.map_err(|e| {
        // The task may have moved the folder before it failed.
        changed(state, DataScope::Instances);
        join_error(e)
    })?;
    match result {
        Ok(()) => {}
        Err(e @ Error::InstancePartlyMoved { .. }) => {
            changed(state, DataScope::Instances);
            return Err(error_dto(&e));
        }
        Err(e) => {
            return Err(match trash_failure {
                Some(detail) => with_id(ErrorCode::RecycleBinFailed, &id, detail),
                None => id_error(&e, &id),
            })
        }
    }
    let (settings, gone) = (state.settings.clone(), id.clone());
    let cleared = tauri::async_runtime::spawn_blocking(move || {
        settings.update(|s| {
            if s.selected_instance.as_deref() == Some(gone.as_str()) {
                s.selected_instance = None;
            }
            Ok(())
        })
    })
    .await;
    changed(state, DataScope::Instances);
    match cleared {
        Ok(Ok(_)) => changed(state, DataScope::Settings),
        // The instance is gone either way; a selection that names nothing is handled by the list.
        Ok(Err(e)) => eprintln!(
            "instances: the selection could not be cleared: {}",
            error_dto(&e).detail
        ),
        Err(_) => {}
    }
    Ok(())
}

#[tauri::command]
pub fn loader_kinds() -> Result<Vec<LoaderInfoDto>, ErrorDto> {
    Ok(loader_kinds_impl())
}

#[tauri::command]
pub async fn loader_versions(
    state: State<'_, AppState>,
    kind: LoaderKindDto,
    game: String,
) -> Result<LoaderVersionsDto, ErrorDto> {
    loader_versions_impl(&state, kind, &game).await
}

#[tauri::command]
pub async fn instances_list(state: State<'_, AppState>) -> Result<Vec<InstanceDto>, ErrorDto> {
    instances_list_impl(&state).await
}

#[tauri::command]
pub async fn instance_create(
    state: State<'_, AppState>,
    req: CreateInstanceRequest,
) -> Result<InstanceDto, ErrorDto> {
    instance_create_impl(&state, req).await
}

#[tauri::command]
pub async fn instance_select(state: State<'_, AppState>, id: String) -> Result<(), ErrorDto> {
    instance_select_impl(&state, id).await
}

#[tauri::command]
pub async fn instance_delete(state: State<'_, AppState>, id: String) -> Result<(), ErrorDto> {
    instance_delete_impl(&state, id).await
}

#[cfg(test)]
mod tests;
