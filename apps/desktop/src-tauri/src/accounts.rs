//! Accounts and sign-in for the desktop app: listing, switching, signing out and the two sign-in
//! flows as cancellable operations.
//!
//! Tokens live in the core's secret store (the Windows Credential Manager) and nowhere else; the
//! types here hand the web view names, ids and flags, never a token, the authorization URL or the
//! device code.

use crate::dto::accounts::{
    AccountDto, AccountKindDto, AccountsDto, LoginEvent, LoginMethod, OperationId, DEV_ACCOUNT_ID,
};
use crate::dto::error::{ErrorCode, ErrorDto};
use crate::error::{error_dto, strip_paths};
use crate::events::{AppEvent, DataScope, EventHub, EventSink};
use crate::settings::{Language, Settings, SettingsHandle};
use hengst_core::args::Account;
use hengst_core::auth::browser::{BrowserLoginPages, BROWSER_LOGIN_TIMEOUT};
use hengst_core::auth::flow::{launch_account_with, LoginSession};
use hengst_core::auth::store::{load_accounts_file, AccountStore, SecretStore, StoredAccount};
use hengst_core::auth::{unix_now, AuthError, MicrosoftAuth};
use hengst_core::paths::Paths;
use hengst_core::secret::Secret;
use hengst_core::{Error, Result};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::{AbortHandle, JoinHandle};

/// Opens an address in the system browser. The text of a failure is never shown (it could carry
/// the address); a refusal is just a refusal.
pub type BrowserOpener = Arc<dyn Fn(&str) -> std::result::Result<(), ()> + Send + Sync>;

/// The opener of a real run: the opener plugin's `open_url`, the code path `OpenerExt::open_url`
/// takes. On Windows the plugin calls `ShellExecuteExW` with the address as the file to open (its
/// `shellexecute-on-windows` feature): no `cmd.exe` and no command line built by this app. The
/// address then reaches the registered browser as one argument of that browser's own command
/// line, which cannot be avoided when handing a URL to a browser.
pub fn system_browser_opener() -> BrowserOpener {
    https_only(|url| tauri_plugin_opener::open_url(url, None::<&str>).map_err(|_| ()))
}

/// Lets only `https://` addresses through to `open`: `ShellExecuteExW` would also run a file path
/// or start a custom protocol handler. The sign-in address is always Microsoft's `https` page.
fn https_only(
    open: impl Fn(&str) -> std::result::Result<(), ()> + Send + Sync + 'static,
) -> BrowserOpener {
    Arc::new(move |url| {
        if url.starts_with("https://") {
            open(url)
        } else {
            Err(())
        }
    })
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

/// A background task panicked or was cancelled: nothing the user can act on.
pub fn internal(what: &str) -> ErrorDto {
    ErrorDto {
        code: ErrorCode::Internal,
        params: BTreeMap::new(),
        action: None,
        detail: what.to_string(),
    }
}

/// The texts of the page the browser shows after the Microsoft redirect. They are fixed here, not
/// taken from the web view: nothing a compromised page sends can end up on the page of a real
/// sign-in. The success text only says that the user can return to the launcher: the Xbox and
/// game steps still follow and can fail.
pub fn login_pages(language: Language) -> BrowserLoginPages {
    let t = |a: &str, b: &str, c: &str, d: &str| BrowserLoginPages {
        success_title: a.to_string(),
        success_body: b.to_string(),
        failure_title: c.to_string(),
        failure_body: d.to_string(),
    };
    match language {
        Language::De => t(
            "Microsoft-Anmeldung abgeschlossen",
            "Du kannst zu Hengst Client zurückkehren und diesen Tab schließen. Dort geht es automatisch weiter.",
            "Anmeldung nicht abgeschlossen",
            "Kehre zu Hengst Client zurück. Dort steht, was passiert ist, und du kannst es erneut versuchen.",
        ),
        Language::En => t(
            "Microsoft sign-in finished",
            "You can return to Hengst Client and close this tab. It continues there on its own.",
            "Sign-in not completed",
            "Return to Hengst Client. It shows what happened, and you can try again.",
        ),
    }
}

// ---- operations ----

/// The kind of a running operation. Browser and device-code sign-in share `Login`, so "use a code
/// instead" ends the waiting browser sign-in and frees its port. `Play` is the preparation of a
/// game start (`play`); it never replaces another one, `play` refuses while one runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    Login,
    Play,
}

struct Entry {
    kind: OpKind,
    abort: AbortHandle,
    on_cancel: Box<dyn Fn() + Send + Sync>,
    /// The authorization URL of a browser sign-in. Never leaves Rust.
    reopen: Option<String>,
    /// When the browser was last asked to open that address.
    opened_at: Option<Instant>,
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
                    opened_at: None,
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
        self.cancel_if(id, |_| true)
    }

    /// `cancel`, but only an operation of this kind.
    pub fn cancel_of(&self, kind: OpKind, id: OperationId) -> bool {
        self.cancel_if(id, |k| k == kind)
    }

    fn cancel_if(&self, id: OperationId, kind: impl Fn(OpKind) -> bool) -> bool {
        let entry = {
            let mut inner = lock(&self.inner);
            match inner.running.get(&id) {
                Some(e) if !e.committing && kind(e.kind) => inner.running.remove(&id),
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

    /// How many operations are in the table.
    #[cfg(test)]
    fn len(&self) -> usize {
        lock(&self.inner).running.len()
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

    /// Remembers the address of the running operation and that the browser was just asked to open it.
    pub fn set_reopen(&self, id: OperationId, url: String) {
        if let Some(entry) = lock(&self.inner).running.get_mut(&id) {
            entry.reopen = Some(url);
            entry.opened_at = Some(Instant::now());
        }
    }

    /// For the Rust side only: the address never goes into a DTO. A second request within `gap`
    /// of the last open is `TooSoon`.
    fn take_reopen(&self, id: OperationId, now: Instant, gap: Duration) -> Reopen {
        let mut inner = lock(&self.inner);
        let Some(entry) = inner.running.get_mut(&id) else {
            return Reopen::Unknown;
        };
        let Some(url) = entry.reopen.clone() else {
            return Reopen::Unknown;
        };
        if entry
            .opened_at
            .is_some_and(|at| now.saturating_duration_since(at) < gap)
        {
            return Reopen::TooSoon;
        }
        entry.opened_at = Some(now);
        Reopen::Url(url)
    }

    /// The operation is about to save its result: from now on it cannot be cancelled. `false`
    /// when it was cancelled in the meantime, then it must stop without sending anything.
    pub(crate) fn try_commit(&self, id: OperationId) -> bool {
        match lock(&self.inner).running.get_mut(&id) {
            Some(entry) => {
                entry.committing = true;
                true
            }
            None => false,
        }
    }

    /// An operation of `kind` is saving its result right now. For `Play` that is the commit
    /// window: the game process is being created and the state still says `Preparing`.
    pub fn has_committing(&self, kind: OpKind) -> bool {
        lock(&self.inner)
            .running
            .values()
            .any(|e| e.kind == kind && e.committing)
    }

    /// The operation ends by itself. `true`: it owns the terminal event and must send it. `false`:
    /// `cancel` was first and has sent `Cancelled`.
    pub(crate) fn finish(&self, id: OperationId) -> bool {
        lock(&self.inner).running.remove(&id).is_some()
    }
}

enum Reopen {
    Unknown,
    TooSoon,
    Url(String),
}

/// The shortest time between two opens of the sign-in page for one operation.
const REOPEN_GAP: Duration = Duration::from_secs(2);

/// The events of one operation, in order, to the sink behind it.
///
/// Events go through a queue that one forwarding task drains into the sink, so the sink is never
/// called while any lock is held (a sink that blocks cannot stall `cancel`). The queue closes
/// with the terminal event, under the same small lock `progress` takes: an event of the task
/// after the terminal event is refused, however far the task still runs before its abort
/// takes effect. `Started` is queued before the task exists, so it is first by ordering.
pub struct OpOut<E = LoginEvent> {
    queue: Arc<Mutex<Option<mpsc::UnboundedSender<E>>>>,
}

impl<E> Clone for OpOut<E> {
    fn clone(&self) -> Self {
        OpOut {
            queue: self.queue.clone(),
        }
    }
}

impl<E: Send + 'static> OpOut<E> {
    /// The forwarding task ends once the terminal event has been handed to `sink`.
    pub fn new(sink: EventSink<E>) -> (OpOut<E>, JoinHandle<()>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let forwarder = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // A panicking sink loses that one event; it must not end the forwarder, because
                // the queue would then refuse every later event and the operation would believe
                // it had ended without a terminal event ever being sent.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink(event)));
            }
        });
        (
            OpOut {
                queue: Arc::new(Mutex::new(Some(tx))),
            },
            forwarder,
        )
    }

    /// Queues a progress event; `false` (nothing sent) once the operation has ended.
    pub fn progress(&self, event: E) -> bool {
        match lock(&self.queue).as_ref() {
            Some(tx) => tx.send(event).is_ok(),
            None => false,
        }
    }

    /// Queues the terminal event and closes the queue; `false` if it was closed already.
    pub fn end(&self, event: E) -> bool {
        match lock(&self.queue).take() {
            Some(tx) => tx.send(event).is_ok(),
            None => false,
        }
    }

    pub fn is_live(&self) -> bool {
        lock(&self.queue).is_some()
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
    /// was the selected account the first remaining one is selected, otherwise none. The core
    /// decides what a failure halfway leaves (`AccountStore::remove`): nothing changed if the
    /// first delete failed, otherwise the entry is marked "sign in again".
    pub fn remove(&self, id: &str) -> Result<()> {
        let _io = self.io();
        if id == DEV_ACCOUNT_ID {
            return Err(account_not_found(id));
        }
        let account = self.exact(id)?;
        self.store.remove(&account.uuid)?;
        Ok(())
    }

    /// The saved login of this account stopped working (4b: the launch reported `LoginExpired`).
    pub fn mark_sign_in_required(&self, uuid: &str) -> Result<()> {
        let _io = self.io();
        self.exact(uuid)?;
        self.store.mark_sign_in_required(uuid)?;
        Ok(())
    }

    /// The account a game starts with: the development account when it is chosen (`dev-offline`
    /// builds only; other builds never make an offline account, and the core refuses to start one
    /// anyway), otherwise the selected Microsoft account through the core's `launch_account`, which
    /// renews its login when it is due. When Microsoft refuses the renewal (`LoginExpired`), the
    /// account is marked "sign in again" before the error is returned; the flag says so, and the
    /// caller then refreshes the account list. With the account comes the entry as it was when it
    /// was chosen (`None` for the development account), for `still_present` before the spawn.
    pub async fn play_account(
        &self,
        settings: &Settings,
    ) -> (Result<(Account, Option<StoredAccount>)>, bool) {
        if cfg!(feature = "dev-offline") && settings.dev_account {
            return (Ok((Account::offline("Dev"), None)), false);
        }
        let stored = match self.store.find(None) {
            Ok(stored) => stored,
            Err(e) => return (Err(e), false),
        };
        // Already flagged (also by a sign-out that failed halfway, which can leave the Minecraft
        // token stored): refused before any token is read or renewed; nothing new to mark.
        if stored.sign_in_required {
            return (
                Err(Error::Auth(AuthError::LoginExpired { name: stored.name })),
                false,
            );
        }
        // A renewed session is saved like a sign-in: under `io_lock`, and only if the entry is
        // unchanged (it may have been signed out while the renewal was in flight, fully or
        // halfway, or signed out and in again). The lock is taken inside the synchronous
        // callback, never across an await. The entry as saved is what the start compares with
        // later (`still_present`). The save runs on this worker thread and can block for up to
        // 775 ms in `replace_file` (a held file); moving it to a blocking thread would need an
        // async callback in the core's `launch_account_with` and an owned `Accounts`, so it stays
        // here (a renewal happens at most once per start).
        let renewed: Mutex<Option<StoredAccount>> = Mutex::new(None);
        let result = launch_account_with(
            &self.store,
            self.auth.as_ref(),
            Some(&stored.uuid),
            unix_now(),
            |session| {
                let _io = self.io();
                self.unchanged(&stored)?;
                let saved = self.store.save_session(session)?;
                *renewed.lock().unwrap_or_else(|p| p.into_inner()) = Some(saved);
                Ok(())
            },
        )
        .await
        .map(|account| {
            let chosen = renewed
                .into_inner()
                .unwrap_or_else(|p| p.into_inner())
                .unwrap_or_else(|| stored.clone());
            (account, Some(chosen))
        });
        if let Err(Error::Auth(AuthError::LoginExpired { .. })) = &result {
            // Signed out (and maybe in again) meanwhile: that is the answer, nothing is marked.
            if let Some(other) = self.after_login_expired(&stored) {
                return (Err(other), true);
            }
            return (result, true);
        }
        (result, false)
    }

    /// The account step of a start reported `LoginExpired`. That is also what a sign-out
    /// overlapping that step looks like (its secrets are gone), so the entry is checked again
    /// under `io_lock` against `stored`, the entry the start chose: gone -> its `AccountNotFound`
    /// is returned and nothing is marked (a mark would tell the user to sign in an account they
    /// just removed); there but a different entry (signed out and in again, or flagged by a
    /// half-failed sign-out) -> `LoginExpired` is returned and nothing is marked (the new entry's
    /// login is fine); the same entry -> it is marked "sign in again" (a failure to mark is only
    /// logged) and `None` is returned.
    fn after_login_expired(&self, stored: &StoredAccount) -> Option<Error> {
        let _io = self.io();
        let now = match self.exact(&stored.uuid) {
            Ok(now) => now,
            Err(e) => return Some(e),
        };
        if now != *stored {
            return Some(Error::Auth(AuthError::LoginExpired { name: now.name }));
        }
        if let Err(e) = self.store.mark_sign_in_required(&stored.uuid) {
            self.diag(&format!(
                "play: could not mark the account ({})",
                strip_paths(&e.to_string())
            ));
        }
        None
    }

    /// The account chosen for a start (`chosen`, as it was then) is still the same entry: a
    /// sign-out removed it (`AccountNotFound`), or it differs (`LoginExpired`): a sign-out failed
    /// halfway and left it "sign in again", or it was signed out and signed in again, which
    /// writes a new entry with a new login. The caller holds `io_lock`.
    fn unchanged(&self, chosen: &StoredAccount) -> Result<()> {
        let now = self.exact(&chosen.uuid)?;
        if now != *chosen {
            return Err(Error::Auth(AuthError::LoginExpired { name: now.name }));
        }
        Ok(())
    }

    /// `unchanged` under `io_lock`: checked right before the game process starts, so a sign-out
    /// during a long preparation never launches with the removed account's token in memory.
    pub fn still_present(&self, chosen: &StoredAccount) -> Result<()> {
        let _io = self.io();
        self.unchanged(chosen)
    }

    fn diag(&self, line: &str) {
        (self.log)(line);
    }

    /// Opens the stored authorization URL of a running browser sign-in again. A second request
    /// within two seconds of the last open does nothing.
    pub fn reopen_browser(
        &self,
        ops: &Operations,
        op: OperationId,
    ) -> std::result::Result<(), ErrorDto> {
        self.reopen_browser_at(ops, op, Instant::now())
    }

    fn reopen_browser_at(
        &self,
        ops: &Operations,
        op: OperationId,
        now: Instant,
    ) -> std::result::Result<(), ErrorDto> {
        match ops.take_reopen(op, now, REOPEN_GAP) {
            Reopen::Unknown => Err(invalid_input("op")),
            Reopen::TooSoon => Ok(()),
            Reopen::Url(url) => (self.opener)(&url).map_err(|()| open_failed()),
        }
    }

    /// Starts a sign-in as an operation and returns its id at once. The channel behind `sink` gets
    /// `Started` first, then the progress, then one terminal event, and nothing after it. A running
    /// sign-in is cancelled first.
    pub fn start_login(
        self: &Arc<Self>,
        ops: &Operations,
        events: &EventHub,
        method: LoginMethod,
        pages: BrowserLoginPages,
        settings: SettingsHandle,
        sink: EventSink<LoginEvent>,
    ) -> OperationId {
        let (out, _forwarder) = OpOut::new(sink);
        let on_cancel: Box<dyn Fn() + Send + Sync> = {
            let (out, accounts) = (out.clone(), self.clone());
            Box::new(move || {
                accounts.diag("sign-in: cancelled");
                out.end(LoginEvent::Cancelled);
            })
        };
        let accounts = self.clone();
        let (ops_for_task, events) = (ops.clone(), events.clone());
        ops.start(OpKind::Login, on_cancel, move |op| {
            // Queued before the task is spawned and before the operation is in the table, so
            // nothing (the task, `cancel`, a replacing `start`) can queue an event ahead of it.
            out.progress(LoginEvent::Started { op });
            let ctx = LoginCtx {
                op,
                method,
                pages,
                settings,
                out,
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
    out: OpOut,
    ops: Operations,
    events: EventHub,
}

fn dto(e: Error) -> ErrorDto {
    error_dto(&e)
}

/// Held by the task of a sign-in. If the task ends in any way other than by deciding its own
/// terminal event (a panic, or being dropped after the commit point, which `cancel` refuses to
/// do), the operation still leaves the table and its channel gets `Failed` with the `internal`
/// code. `cancel` removes the entry first and sends `Cancelled` itself, so then nothing is sent.
struct EndGuard {
    ops: Operations,
    op: OperationId,
    out: OpOut,
    armed: bool,
}

impl EndGuard {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for EndGuard {
    fn drop(&mut self) {
        if self.armed && self.ops.finish(self.op) {
            self.out.end(LoginEvent::Failed {
                error: internal("the sign-in task ended without a result"),
            });
        }
    }
}

impl Accounts {
    /// The task of one sign-in: the steps, then the one terminal event if this task owns it.
    async fn drive(self: Arc<Self>, c: LoginCtx) {
        self.diag(&format!("sign-in {}: started ({:?})", c.op.0, c.method));
        let mut guard = EndGuard {
            ops: c.ops.clone(),
            op: c.op,
            out: c.out.clone(),
            armed: true,
        };
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
                    c.out.end(LoginEvent::SignedIn { account });
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
                    c.out.end(LoginEvent::Failed { error });
                }
            }
        }
        guard.disarm();
    }

    /// `Ok(None)`: cancelled meanwhile. The flag says whether saving had started.
    async fn sign_in(
        self: &Arc<Self>,
        c: &LoginCtx,
    ) -> (std::result::Result<Option<AccountDto>, ErrorDto>, bool) {
        let tokens = match self.microsoft_step(c).await {
            Ok(Some(tokens)) => tokens,
            Ok(None) => return (Ok(None), false),
            Err(e) => return (Err(e), false),
        };
        if !c.out.progress(LoginEvent::Completing) {
            return (Ok(None), false);
        }
        let auth = self.auth.as_ref().expect("checked by the Microsoft step");
        let session = match auth.complete_login(tokens, unix_now()).await {
            Ok(session) => session,
            Err(e) => return (Err(dto(e)), false),
        };
        if !c.ops.try_commit(c.op) {
            return (Ok(None), false);
        }
        let (accounts, settings) = (self.clone(), c.settings.clone());
        let saved =
            tokio::task::spawn_blocking(move || accounts.persist(&session, &settings)).await;
        let saved = match saved {
            Ok(saved) => saved.map(Some).map_err(dto),
            Err(_) => Err(internal("the task that saves the account failed")),
        };
        (saved, true)
    }

    /// The browser or device-code step: Microsoft tokens for the signed-in user. `None`: the
    /// operation ended (cancelled or replaced) meanwhile.
    async fn microsoft_step(
        self: &Arc<Self>,
        c: &LoginCtx,
    ) -> std::result::Result<Option<hengst_core::auth::MsaTokens>, ErrorDto> {
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
                let url = login.authorize_url().to_string();
                c.ops.set_reopen(c.op, url.clone());
                let (opener, out) = (self.opener.clone(), c.out.clone());
                let opened =
                    tokio::task::spawn_blocking(move || open_unless_ended(&out, &opener, &url))
                        .await
                        .map_err(|_| internal("the task that opens the browser failed"))?;
                match opened {
                    Ok(true) => {}
                    // Ended meanwhile: dropping `login` frees the port.
                    Ok(false) => return Ok(None),
                    Err(()) => return Err(open_failed()),
                }
                if !c.out.progress(LoginEvent::BrowserOpened) {
                    return Ok(None);
                }
                auth.finish_browser_login(login, &c.pages, BROWSER_LOGIN_TIMEOUT)
                    .await
                    .map(Some)
                    .map_err(dto)
            }
            LoginMethod::DeviceCode => {
                let code = auth.start_device_login().await.map_err(dto)?;
                let shown = c.out.progress(LoginEvent::DeviceCode {
                    user_code: code.user_code.clone(),
                    verification_uri: code.verification_uri.clone(),
                    expires_at: unix_now().saturating_add(code.expires_in),
                });
                if !shown {
                    return Ok(None);
                }
                auth.poll_device_login(&code).await.map(Some).map_err(dto)
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

/// Opens `url` unless the operation has ended already (a replaced sign-in must not open a tab
/// after its replacement started). `Ok(false)`: not opened because it ended.
fn open_unless_ended(
    out: &OpOut,
    opener: &BrowserOpener,
    url: &str,
) -> std::result::Result<bool, ()> {
    if !out.is_live() {
        return Ok(false);
    }
    opener(url).map(|()| true)
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
