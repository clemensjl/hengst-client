//! Accounts and sign-in for the desktop app: listing, switching, signing out and the two sign-in
//! flows as cancellable operations.
//!
//! Tokens live in the core's secret store (the Windows Credential Manager) and nowhere else; the
//! types here hand the web view names, ids and flags, never a token, the authorization URL or the
//! device code.

use crate::dto::accounts::{
    AccountDto, AccountKindDto, AccountsDto, LoginEvent, LoginMethod, LoginPagesDto, OperationId,
    DEV_ACCOUNT_ID,
};
use crate::dto::error::{ErrorCode, ErrorDto};
use crate::error::error_dto;
use crate::events::{AppEvent, DataScope, EventHub, EventSink};
use crate::settings::{Settings, SettingsHandle};
use hengst_core::auth::browser::{BrowserLoginPages, BROWSER_LOGIN_TIMEOUT};
use hengst_core::auth::flow::LoginSession;
use hengst_core::auth::store::{load_accounts_file, AccountStore, SecretStore, StoredAccount};
use hengst_core::auth::{unix_now, AuthError, MicrosoftAuth};
use hengst_core::paths::Paths;
use hengst_core::secret::Secret;
use hengst_core::{Error, Result};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::task::AbortHandle;

/// Opens an address in the system browser. The text of a failure is never shown (it could carry
/// the address); a refusal is just a refusal.
pub type BrowserOpener = Arc<dyn Fn(&str) -> std::result::Result<(), ()> + Send + Sync>;

/// The opener of a real run: the opener plugin's `open_url`, the code path `OpenerExt::open_url`
/// takes. On Windows the plugin calls `ShellExecuteExW` with the address as the file to open (its
/// `shellexecute-on-windows` feature): no `cmd.exe` and no command line built by this app. The
/// address then reaches the registered browser as one argument of that browser's own command
/// line, which cannot be avoided when handing a URL to a browser.
pub fn system_browser_opener() -> BrowserOpener {
    Arc::new(|url| tauri_plugin_opener::open_url(url, None::<&str>).map_err(|_| ()))
}

/// The opener of the `e2e` build: the test kit plays the browser against the mock service in
/// its own task. The address goes nowhere else.
#[cfg(feature = "e2e")]
pub fn visiting_browser_opener() -> BrowserOpener {
    Arc::new(|url| {
        let url = url.to_string();
        tauri::async_runtime::spawn(async move {
            let _ = hengst_testkit::browser_visit(&url).await;
        });
        Ok(())
    })
}

/// A secret store that refuses everything: the credential manager could not be opened.
struct UnavailableSecretStore(String);

impl UnavailableSecretStore {
    fn error(&self) -> Error {
        Error::CredentialStore(self.0.clone())
    }
}

impl SecretStore for UnavailableSecretStore {
    fn get(&self, _: &str) -> Result<Option<Secret>> {
        Err(self.error())
    }

    fn set(&self, _: &str, _: &Secret) -> Result<()> {
        Err(self.error())
    }

    fn delete(&self, _: &str) -> Result<()> {
        Err(self.error())
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn account_not_found(id: &str) -> Error {
    Error::Auth(AuthError::AccountNotFound {
        selector: id.to_string(),
    })
}

fn invalid_input(field: &str) -> ErrorDto {
    ErrorDto {
        code: ErrorCode::InvalidInput,
        params: BTreeMap::from([("field".to_string(), field.to_string())]),
        action: None,
        detail: format!("invalid value for {field}"),
    }
}

fn open_failed() -> ErrorDto {
    ErrorDto {
        code: ErrorCode::OpenFailed,
        params: BTreeMap::new(),
        action: None,
        detail: "the system could not open the default browser".to_string(),
    }
}

/// The longest page text accepted from the web view.
const MAX_PAGE_TEXT: usize = 500;

/// The texts of the browser page, checked: not empty, not huge. The core escapes them.
pub fn browser_pages(pages: LoginPagesDto) -> std::result::Result<BrowserLoginPages, ErrorDto> {
    let LoginPagesDto {
        success_title,
        success_body,
        failure_title,
        failure_body,
    } = pages;
    for text in [&success_title, &success_body, &failure_title, &failure_body] {
        if text.trim().is_empty() || text.chars().count() > MAX_PAGE_TEXT {
            return Err(invalid_input("pages"));
        }
    }
    Ok(BrowserLoginPages {
        success_title,
        success_body,
        failure_title,
        failure_body,
    })
}

// ---- operations ----

/// The kind of a running operation. Browser and device-code sign-in share `Login`, so "use a code
/// instead" ends the waiting browser sign-in and frees its port. Phase 4b adds `Play`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    Login,
}

struct Entry {
    kind: OpKind,
    abort: AbortHandle,
    on_cancel: Box<dyn Fn() + Send + Sync>,
    /// The authorization URL of a browser sign-in. Never leaves Rust.
    reopen: Option<String>,
    /// The operation saves its result and can no longer be cancelled.
    committing: bool,
}

#[derive(Default)]
struct OpsInner {
    last: u64,
    running: HashMap<OperationId, Entry>,
}

/// The running operations. An operation is in the table from `start` until the moment its
/// terminal event is decided: whoever removes it from the table (the operation itself through
/// `finish`, or `cancel`) sends that one terminal event, so there is exactly one.
#[derive(Clone, Default)]
pub struct Operations {
    inner: Arc<Mutex<OpsInner>>,
}

impl Operations {
    /// Gives the operation its id, ends a running one of the same kind (abort and `on_cancel`, which
    /// sends `Cancelled` to that operation's channel), then lets `spawn` start the task. An
    /// operation that is already saving its result is left alone.
    pub fn start(
        &self,
        kind: OpKind,
        on_cancel: Box<dyn Fn() + Send + Sync>,
        spawn: impl FnOnce(OperationId) -> AbortHandle,
    ) -> OperationId {
        let (id, replaced) = {
            let mut inner = lock(&self.inner);
            let old: Vec<OperationId> = inner
                .running
                .iter()
                .filter(|(_, e)| e.kind == kind && !e.committing)
                .map(|(id, _)| *id)
                .collect();
            let replaced: Vec<Entry> = old
                .into_iter()
                .filter_map(|id| inner.running.remove(&id))
                .collect();
            inner.last += 1;
            let id = OperationId(inner.last);
            // The table is held while the task is created: the task cannot finish before it is
            // in the table, because `finish` takes the same lock.
            let abort = spawn(id);
            inner.running.insert(
                id,
                Entry {
                    kind,
                    abort,
                    on_cancel,
                    reopen: None,
                    committing: false,
                },
            );
            (id, replaced)
        };
        for entry in replaced {
            entry.abort.abort();
            (entry.on_cancel)();
        }
        id
    }

    /// Ends an operation that has not started saving its result: abort (dropping the task frees its
    /// listener) and `Cancelled` to its channel. `false` for an unknown, finished or saving one.
    pub fn cancel(&self, id: OperationId) -> bool {
        let entry = {
            let mut inner = lock(&self.inner);
            match inner.running.get(&id) {
                Some(e) if !e.committing => inner.running.remove(&id),
                _ => None,
            }
        };
        match entry {
            Some(entry) => {
                entry.abort.abort();
                (entry.on_cancel)();
                true
            }
            None => false,
        }
    }

    /// Cancels everything that can still be cancelled (the window is closing).
    pub fn cancel_all(&self) {
        let entries: Vec<Entry> = {
            let mut inner = lock(&self.inner);
            let ids: Vec<OperationId> = inner
                .running
                .iter()
                .filter(|(_, e)| !e.committing)
                .map(|(id, _)| *id)
                .collect();
            ids.into_iter()
                .filter_map(|id| inner.running.remove(&id))
                .collect()
        };
        for entry in entries {
            entry.abort.abort();
            (entry.on_cancel)();
        }
    }

    pub fn set_reopen(&self, id: OperationId, url: String) {
        if let Some(entry) = lock(&self.inner).running.get_mut(&id) {
            entry.reopen = Some(url);
        }
    }

    /// For the Rust side only: the address never goes into a DTO.
    pub fn reopen_url(&self, id: OperationId) -> Option<String> {
        lock(&self.inner)
            .running
            .get(&id)
            .and_then(|e| e.reopen.clone())
    }

    /// The operation is about to save its result: from now on it cannot be cancelled. `false`
    /// when it was cancelled in the meantime, then it must stop without sending anything.
    fn try_commit(&self, id: OperationId) -> bool {
        match lock(&self.inner).running.get_mut(&id) {
            Some(entry) => {
                entry.committing = true;
                true
            }
            None => false,
        }
    }

    /// The operation ends by itself. `true`: it owns the terminal event and must send it. `false`:
    /// `cancel` was first and has sent `Cancelled`.
    fn finish(&self, id: OperationId) -> bool {
        lock(&self.inner).running.remove(&id).is_some()
    }
}

// ---- accounts ----

/// The accounts of the app: the core's account store behind one lock for the file, plus the two
/// sign-in flows.
pub struct Accounts {
    paths: Paths,
    store: AccountStore<Box<dyn SecretStore>>,
    /// Serializes every write of `accounts.json` (and the settings change that belongs to it).
    /// Never held across an await.
    io_lock: Mutex<()>,
    auth: Option<MicrosoftAuth>,
    opener: BrowserOpener,
    /// Set when the credential manager could not be opened: sign-in refuses before it starts.
    store_error: Option<String>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Accounts {
    pub fn new(
        paths: &Paths,
        secrets: Box<dyn SecretStore>,
        auth: Option<MicrosoftAuth>,
        opener: BrowserOpener,
    ) -> Accounts {
        Accounts {
            paths: paths.clone(),
            store: AccountStore::new(paths.accounts_file(), secrets),
            io_lock: Mutex::new(()),
            auth,
            opener,
            store_error: None,
            log: Arc::new(|line| eprintln!("{line}")),
        }
    }

    /// Accounts without a usable secret store: the list works, sign-in and sign-out report
    /// `credentialStore`.
    pub fn without_secret_store(
        paths: &Paths,
        reason: String,
        auth: Option<MicrosoftAuth>,
        opener: BrowserOpener,
    ) -> Accounts {
        let mut accounts = Accounts::new(
            paths,
            Box::new(UnavailableSecretStore(reason.clone())),
            auth,
            opener,
        );
        accounts.store_error = Some(reason);
        accounts
    }

    /// Replaces where diagnostic lines go (tests capture them). Lines name ids, methods and error
    /// codes, nothing else.
    pub fn with_log(mut self, log: Arc<dyn Fn(&str) + Send + Sync>) -> Accounts {
        self.log = log;
        self
    }

    pub fn can_sign_in(&self) -> bool {
        self.auth.is_some()
    }

    fn io(&self) -> MutexGuard<'_, ()> {
        lock(&self.io_lock)
    }

    /// Reads `accounts.json` only, never the secret store.
    pub fn list(&self, settings: &Settings) -> Result<AccountsDto> {
        let file = load_accounts_file(&self.paths.accounts_file())?;
        let dev_selected = cfg!(feature = "dev-offline") && settings.dev_account;
        let mut accounts = Vec::new();
        if cfg!(feature = "dev-offline") {
            accounts.push(AccountDto {
                id: DEV_ACCOUNT_ID.to_string(),
                name: "Dev".to_string(),
                kind: AccountKindDto::OfflineDev,
                selected: dev_selected,
                sign_in_required: false,
            });
        }
        accounts.extend(file.accounts.iter().map(|a| {
            account_dto(
                a,
                !dev_selected && file.default.as_deref() == Some(a.uuid.as_str()),
            )
        }));
        Ok(AccountsDto {
            accounts,
            can_sign_in: self.can_sign_in(),
        })
    }

    /// Chooses the account the game starts with. The setting is saved in the same call.
    pub fn select(&self, id: &str, settings: &SettingsHandle) -> Result<()> {
        let _io = self.io();
        if id == DEV_ACCOUNT_ID {
            if !cfg!(feature = "dev-offline") {
                return Err(account_not_found(id));
            }
            settings.update(|s| {
                s.dev_account = true;
                Ok(())
            })?;
            return Ok(());
        }
        self.exact(id)?;
        self.store.set_default(id)?;
        if settings.get().dev_account {
            settings.update(|s| {
                s.dev_account = false;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// The stored account with exactly this UUID (names do not select here).
    fn exact(&self, id: &str) -> Result<StoredAccount> {
        let file = load_accounts_file(&self.paths.accounts_file())?;
        file.accounts
            .into_iter()
            .find(|a| a.uuid == id)
            .ok_or_else(|| account_not_found(id))
    }

    /// Signs out: removes the tokens from the secret store and the entry from the file; when it
    /// was the selected account the first remaining one is selected, otherwise none. If the secret
    /// store refuses halfway, the entry stays and is marked "sign in again", so it is never shown
    /// as usable without its tokens.
    pub fn remove(&self, id: &str) -> Result<()> {
        let _io = self.io();
        if id == DEV_ACCOUNT_ID {
            return Err(account_not_found(id));
        }
        let account = self.exact(id)?;
        match self.store.remove(&account.uuid) {
            Ok(_) => Ok(()),
            Err(e) => {
                // The file still lists the account; tokens may be gone already. Best effort: a
                // failing write here must not hide the original error.
                let _ = self.store.mark_sign_in_required(&account.uuid);
                Err(e)
            }
        }
    }

    /// The saved login of this account stopped working (4b: the launch reported `LoginExpired`).
    pub fn mark_sign_in_required(&self, uuid: &str) -> Result<()> {
        let _io = self.io();
        self.exact(uuid)?;
        self.store.mark_sign_in_required(uuid)?;
        Ok(())
    }

    fn diag(&self, line: &str) {
        (self.log)(line);
    }

    /// Opens the stored authorization URL of a running browser sign-in again.
    pub fn reopen_browser(
        &self,
        ops: &Operations,
        op: OperationId,
    ) -> std::result::Result<(), ErrorDto> {
        let url = ops.reopen_url(op).ok_or_else(|| invalid_input("op"))?;
        (self.opener)(&url).map_err(|()| open_failed())
    }

    /// Starts a sign-in as an operation and returns its id at once. The channel behind `sink` gets
    /// `Started` first (before this returns), then the progress, then one terminal event. A
    /// running sign-in is cancelled first.
    pub fn start_login(
        self: &Arc<Self>,
        ops: &Operations,
        events: &EventHub,
        method: LoginMethod,
        pages: BrowserLoginPages,
        settings: SettingsHandle,
        sink: EventSink<LoginEvent>,
    ) -> OperationId {
        let on_cancel: Box<dyn Fn() + Send + Sync> = {
            let (sink, accounts) = (sink.clone(), self.clone());
            Box::new(move || {
                accounts.diag("sign-in: cancelled");
                sink(LoginEvent::Cancelled);
            })
        };
        let accounts = self.clone();
        let (ops_for_task, events) = (ops.clone(), events.clone());
        ops.start(OpKind::Login, on_cancel, move |op| {
            sink(LoginEvent::Started { op });
            let ctx = LoginCtx {
                op,
                method,
                pages,
                settings,
                sink,
                ops: ops_for_task,
                events,
            };
            tokio::spawn(accounts.drive(ctx)).abort_handle()
        })
    }
}

fn account_dto(a: &StoredAccount, selected: bool) -> AccountDto {
    AccountDto {
        id: a.uuid.clone(),
        name: a.name.clone(),
        kind: AccountKindDto::Microsoft,
        selected,
        sign_in_required: a.sign_in_required,
    }
}

struct LoginCtx {
    op: OperationId,
    method: LoginMethod,
    pages: BrowserLoginPages,
    settings: SettingsHandle,
    sink: EventSink<LoginEvent>,
    ops: Operations,
    events: EventHub,
}

fn dto(e: Error) -> ErrorDto {
    error_dto(&e)
}

impl Accounts {
    /// The task of one sign-in: the steps, then the one terminal event if this task owns it.
    async fn drive(self: Arc<Self>, c: LoginCtx) {
        self.diag(&format!("sign-in {}: started ({:?})", c.op.0, c.method));
        let (result, committed) = self.sign_in(&c).await;
        match result {
            // `cancel` was first and has sent `Cancelled`.
            Ok(None) => {}
            Ok(Some(account)) => {
                if c.ops.finish(c.op) {
                    self.diag(&format!("sign-in {}: signed in", c.op.0));
                    c.events.emit(AppEvent::DataChanged {
                        scope: DataScope::Accounts,
                    });
                    c.events.emit(AppEvent::DataChanged {
                        scope: DataScope::Settings,
                    });
                    (c.sink)(LoginEvent::SignedIn { account });
                }
            }
            Err(error) => {
                if c.ops.finish(c.op) {
                    self.diag(&format!("sign-in {}: failed ({:?})", c.op.0, error.code));
                    if committed {
                        // Saving may have changed the file before it failed.
                        c.events.emit(AppEvent::DataChanged {
                            scope: DataScope::Accounts,
                        });
                    }
                    (c.sink)(LoginEvent::Failed { error });
                }
            }
        }
    }

    /// `Ok(None)`: cancelled meanwhile. The flag says whether saving had started.
    async fn sign_in(
        self: &Arc<Self>,
        c: &LoginCtx,
    ) -> (std::result::Result<Option<AccountDto>, ErrorDto>, bool) {
        let tokens = match self.microsoft_step(c).await {
            Ok(tokens) => tokens,
            Err(e) => return (Err(e), false),
        };
        (c.sink)(LoginEvent::Completing);
        let auth = self.auth.as_ref().expect("checked by the Microsoft step");
        let session = match auth.complete_login(tokens, unix_now()).await {
            Ok(session) => session,
            Err(e) => return (Err(dto(e)), false),
        };
        if !c.ops.try_commit(c.op) {
            return (Ok(None), false);
        }
        let (accounts, settings) = (self.clone(), c.settings.clone());
        let saved = tokio::task::spawn_blocking(move || accounts.persist(&session, &settings))
            .await
            .unwrap_or_else(|e| Err(Error::Io(std::io::Error::other(e.to_string()))));
        (saved.map(Some).map_err(dto), true)
    }

    /// The browser or device-code step: Microsoft tokens for the signed-in user.
    async fn microsoft_step(
        self: &Arc<Self>,
        c: &LoginCtx,
    ) -> std::result::Result<hengst_core::auth::msa::MsaTokens, ErrorDto> {
        if let Some(reason) = &self.store_error {
            return Err(dto(Error::CredentialStore(reason.clone())));
        }
        let auth = self
            .auth
            .as_ref()
            .ok_or_else(|| dto(Error::Auth(AuthError::MissingClientId)))?;
        match c.method {
            LoginMethod::Browser => {
                let login = auth.start_browser_login().await.map_err(dto)?;
                c.ops.set_reopen(c.op, login.authorize_url().to_string());
                let (opener, url) = (self.opener.clone(), login.authorize_url().to_string());
                let opened = tokio::task::spawn_blocking(move || opener(&url))
                    .await
                    .unwrap_or(Err(()));
                if opened.is_err() {
                    // Dropping `login` frees the port.
                    return Err(open_failed());
                }
                (c.sink)(LoginEvent::BrowserOpened);
                auth.finish_browser_login(login, &c.pages, BROWSER_LOGIN_TIMEOUT)
                    .await
                    .map_err(dto)
            }
            LoginMethod::DeviceCode => {
                let code = auth.start_device_login().await.map_err(dto)?;
                (c.sink)(LoginEvent::DeviceCode {
                    user_code: code.user_code.clone(),
                    verification_uri: code.verification_uri.clone(),
                    expires_at: unix_now().saturating_add(code.expires_in),
                });
                auth.poll_device_login(&code).await.map_err(dto)
            }
        }
    }

    /// Saves the session, selects the account and clears the development-account choice. Blocking.
    fn persist(&self, session: &LoginSession, settings: &SettingsHandle) -> Result<AccountDto> {
        let _io = self.io();
        let stored = self.store.save_session(session)?;
        self.store.set_default(&stored.uuid)?;
        settings.update(|s| {
            s.dev_account = false;
            Ok(())
        })?;
        Ok(account_dto(&stored, true))
    }
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
