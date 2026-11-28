//! Playing: one game at a time, prepared as a cancellable operation, then watched until it exits.
//!
//! States, each change sent as `AppEvent::GameState`:
//!
//! ```text
//! Idle { last_exit }  --play-->                               Preparing
//! Preparing           --failed / cancelled / task ended-->    Idle { last_exit: None }
//! Preparing           --process started-->                    Running
//! Running             --game_stop reached a window-->         Stopping
//! Running, Stopping   --game_force_stop-->                    Stopping
//! Running, Stopping   --exit (or the watcher ended)-->        Idle { last_exit: Some(..) }
//! ```
//!
//! - `Preparing` is entered under `AppState::instance_lock` (with `active_instance` set), so a
//!   delete cannot slip in between the check and the start. `last_exit` is dropped there.
//! - Every way out of `Preparing` goes back to `Idle { last_exit: None }`: a failure of any step
//!   (account, preparation, process start), `play_cancel` or a cancelled operation (page reload,
//!   window destroyed: `Operations::cancel_all`), and a task that ends without a result (a panic).
//!   Whoever removes the operation from the table owns the move and the terminal event.
//! - Each start has its own `CancelFlag` (a flag stays set once cancelled). Cancelling sets it and
//!   aborts the task: the abort stops downloads at their next await, the flag stops the installer
//!   processors running on a blocking thread (`JobRunner` checks it every 100 ms).
//! - Right before the process starts, the operation commits: from then on it cannot be cancelled,
//!   so a started game is never left unwatched by an abort.
//! - `Running` keeps the `GameProcess` for the whole run (it is the only handle to stop the game).
//!   The exit moves to `Idle { last_exit }`, also when the watching task ends without a result
//!   (code -1), so no state is ever stuck.
//! - When the launcher closes, nothing stops the game: its job has no kill-on-close. A game that
//!   is still being prepared is cancelled with the window (`cancel_all`).
//!
//! Game output: the core calls the line sink on its reader thread while it holds the run's output
//! lock, so the sink only puts the line into an unbounded channel (it never waits and never drops
//! a line; the launcher log file holds the same lines). One task per run drains the channel into
//! the live log (`log.rs`) with the run id; its push never waits for a reader, so the channel
//! drains as fast as lines come. Lines can still arrive after the exit (a child of the game
//! holding the pipe open); they go to their own run's log. When the channel closes, the run's
//! log is complete.

use crate::accounts::{internal, Accounts, OpKind, OpOut, Operations};
use crate::dto::accounts::OperationId;
use crate::dto::error::{ErrorAction, ErrorCode, ErrorDto};
use crate::dto::play::{GameStateDto, LastExit, PlayEvent, RunId, StopResult, TransferDto};
use crate::error::{error_dto, instance_id_error, redact, strip_paths};
use crate::events::{AppEvent, DataScope, EventHub, EventSink};
use crate::log::{LiveLog, RunCloser};
use crate::settings::Settings;
use hengst_core::auth::unix_now;
use hengst_core::game::{spawn_game, GameProcess, LineSink};
use hengst_core::gamelog::LogLine;
use hengst_core::instances::{self, Instance};
use hengst_core::launch::{prepare_with_progress, Endpoints, LaunchOptions, Prepared};
use hengst_core::paths::Paths;
use hengst_core::process::{CancelFlag, JobRunner};
use hengst_core::progress::{Progress, ProgressSink, Transfer};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Component, Path};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// What a preparation gets: the launch, where to report and the flag that stops it.
pub struct PrepareJob {
    pub paths: Paths,
    pub opts: LaunchOptions,
    pub endpoints: Endpoints,
    pub cancel: CancelFlag,
    pub progress: ProgressSink,
}

pub type PrepareFuture = Pin<Box<dyn Future<Output = hengst_core::Result<Prepared>> + Send>>;
/// Prepares a launch. The real one is the core's `prepare_with_progress` with a `JobRunner`;
/// tests replace it to hold a start in its blocking phase.
pub type Preparer = Arc<dyn Fn(PrepareJob) -> PrepareFuture + Send + Sync>;

pub fn core_preparer() -> Preparer {
    Arc::new(|job: PrepareJob| {
        Box::pin(async move {
            let runner = Arc::new(JobRunner::new(job.cancel));
            prepare_with_progress(&job.paths, &job.opts, &job.endpoints, runner, job.progress).await
        })
    })
}

/// Receives every output line of a run, already redacted by the core, with its run id.
pub type LineHandler = Arc<dyn Fn(RunId, LogLine) + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct Run {
    instance: String,
    run_id: RunId,
    since: u64,
    process: Arc<GameProcess>,
    /// `game_stop` calls whose close request is being sent right now.
    stops_in_flight: u32,
    /// A close request reached a window of the game.
    asked: bool,
    /// `game_force_stop` calls whose termination is running right now.
    forcing: u32,
    /// A force stop went through.
    forced: bool,
}

impl Run {
    /// The app asked the game to stop. A request still in flight counts, so an exit that
    /// overtakes its answer is not "unexpected"; one that reached nothing and failed does not.
    fn stop_requested(&self) -> bool {
        self.asked || self.forced || self.forcing > 0 || self.stops_in_flight > 0
    }

    /// Shown as `Stopping`: a stop reached the game or a force stop is under way.
    fn stopping(&self) -> bool {
        self.asked || self.forced || self.forcing > 0
    }
}

enum Phase {
    Idle { last_exit: Option<LastExit> },
    Preparing { op: OperationId, instance: String },
    Running(Run),
}

struct Inner {
    phase: Phase,
    last_run: u64,
}

/// The one game of the app and its state.
pub struct Game {
    inner: Mutex<Inner>,
    events: EventHub,
    /// `AppState::active_instance`: set while preparing or running, cleared in `Idle`, always
    /// changed together with the phase (under `inner`).
    active: Arc<Mutex<Option<String>>>,
    preparer: Mutex<Preparer>,
    /// An extra listener for every line (tests); the live log gets them anyway.
    lines: Mutex<Option<LineHandler>>,
    log: Arc<LiveLog>,
}

impl Game {
    pub fn new(events: EventHub, active: Arc<Mutex<Option<String>>>) -> Game {
        Game {
            inner: Mutex::new(Inner {
                phase: Phase::Idle { last_exit: None },
                last_run: 0,
            }),
            events,
            active,
            preparer: Mutex::new(core_preparer()),
            lines: Mutex::new(None),
            log: Arc::new(LiveLog::new()),
        }
    }

    /// The live log the runs feed.
    pub fn log(&self) -> &Arc<LiveLog> {
        &self.log
    }

    /// Replaces how launches are prepared (tests).
    pub fn set_preparer(&self, preparer: Preparer) {
        *lock(&self.preparer) = preparer;
    }

    /// Also hands every output line to `handler` from now on, besides the live log (tests).
    pub fn set_line_handler(&self, handler: LineHandler) {
        *lock(&self.lines) = Some(handler);
    }

    pub fn state(&self) -> GameStateDto {
        dto(&lock(&self.inner).phase)
    }

    /// Moves to `phase`, keeps `active_instance` in step and tells the web view.
    fn set(&self, inner: &mut Inner, phase: Phase) {
        *lock(&self.active) = match &phase {
            Phase::Idle { .. } => None,
            Phase::Preparing { instance, .. } => Some(instance.clone()),
            Phase::Running(run) => Some(run.instance.clone()),
        };
        inner.phase = phase;
        self.emit(inner);
    }

    fn emit(&self, inner: &Inner) {
        self.events.emit(AppEvent::GameState {
            state: dto(&inner.phase),
        });
    }

    /// Leaves `Preparing` of `op` for `Idle` (no exit to report: the start did not happen).
    fn end_preparing(&self, inner: &mut Inner, op: OperationId) {
        if matches!(inner.phase, Phase::Preparing { op: o, .. } if o == op) {
            self.set(inner, Phase::Idle { last_exit: None });
        }
    }

    /// The process of `run_id` has exited (`code` -1 when unknown).
    fn ended(&self, run_id: RunId, code: i32) {
        let mut inner = lock(&self.inner);
        let Phase::Running(run) = &inner.phase else {
            return;
        };
        if run.run_id != run_id {
            return;
        }
        let last_exit = LastExit {
            instance: run.instance.clone(),
            run_id,
            code,
            stop_requested: run.stop_requested(),
        };
        self.log.run_ended(run_id, code, last_exit.stop_requested);
        self.set(
            &mut inner,
            Phase::Idle {
                last_exit: Some(last_exit),
            },
        );
    }
}

fn dto(phase: &Phase) -> GameStateDto {
    match phase {
        Phase::Idle { last_exit } => GameStateDto::Idle {
            last_exit: last_exit.clone(),
        },
        Phase::Preparing { instance, .. } => GameStateDto::Preparing {
            instance: instance.clone(),
        },
        Phase::Running(run) if run.stopping() => GameStateDto::Stopping {
            instance: run.instance.clone(),
            run_id: run.run_id,
        },
        Phase::Running(run) => GameStateDto::Running {
            instance: run.instance.clone(),
            run_id: run.run_id,
            since: run.since,
        },
    }
}

fn busy() -> ErrorDto {
    ErrorDto {
        code: ErrorCode::Busy,
        params: BTreeMap::new(),
        action: None,
        detail: "a game is already being prepared or running".to_string(),
    }
}

impl From<Transfer> for TransferDto {
    fn from(t: Transfer) -> TransferDto {
        match t {
            Transfer::GameFiles => TransferDto::GameFiles,
            Transfer::Assets => TransferDto::Assets,
            Transfer::Java => TransferDto::Java,
        }
    }
}

/// `path` relative to `root` with `/`, if it lies below it (plain names only).
pub(crate) fn relative(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rest
        .components()
        .map(|c| match c {
            Component::Normal(n) => n.to_str(),
            _ => None,
        })
        .collect();
    let parts = parts.filter(|p| !p.is_empty())?;
    Some(parts.join("/"))
}

/// The web view's form of a progress event. No `_` arm: a new core event breaks the build until
/// it is mapped. `None` for an install log outside the data folder, which cannot happen (the
/// core writes it to `<root>/logs`); its absolute path is never sent instead.
pub fn play_event(p: Progress, root: &Path) -> Option<PlayEvent> {
    Some(match p {
        Progress::ResolvingLoader { kind, game } => PlayEvent::ResolvingLoader {
            kind: kind.into(),
            game,
        },
        Progress::DownloadingInstaller { kind } => {
            PlayEvent::DownloadingInstaller { kind: kind.into() }
        }
        Progress::Downloading { what, count } => PlayEvent::Downloading {
            what: what.into(),
            count: u32::try_from(count).unwrap_or(u32::MAX),
        },
        Progress::Downloaded { what } => PlayEvent::Downloaded { what: what.into() },
        Progress::InstallStarted {
            kind,
            version,
            game,
        } => PlayEvent::InstallStarted {
            kind: kind.into(),
            version,
            game,
        },
        Progress::InstallLog { path } => PlayEvent::InstallLog {
            path: relative(root, &path)?,
        },
        Progress::InstallStep { index, total, name } => PlayEvent::InstallStep {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            total: u32::try_from(total).unwrap_or(u32::MAX),
            name,
        },
        Progress::VerifyingInstall => PlayEvent::VerifyingInstall,
        Progress::InstallFinished { elapsed } => PlayEvent::InstallFinished {
            elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        },
        Progress::InstallReused => PlayEvent::InstallReused,
        Progress::Notice { message } => PlayEvent::Notice {
            message: strip_paths(&redact(&message)),
        },
    })
}

/// The web view's form of a failed start: `error_dto`, and for a failed installer step the action
/// "open the log" with the log's path relative to the data folder (as `InstallLog` sends it; the
/// web view hands it back through `resolve_log_path`). A log outside the data folder gets no
/// action, never its absolute path.
pub fn play_error(e: &hengst_core::Error, root: &Path) -> ErrorDto {
    let mut dto = error_dto(e);
    if let hengst_core::Error::Processor { log, .. } = e {
        dto.action = relative(root, log).map(|path| ErrorAction::OpenLog { path });
    }
    dto
}

/// Everything a start needs, taken from `AppState` before the task runs.
pub struct PlayRequest {
    pub game: Arc<Game>,
    pub ops: Operations,
    pub events: EventHub,
    pub accounts: Arc<Accounts>,
    pub paths: Paths,
    pub endpoints: Endpoints,
    pub settings: Settings,
    pub instance: Instance,
    /// The instance's run lock is held: a game of it runs that this session did not start (an
    /// earlier session that crashed or was closed, `hengst_core::runlock`).
    pub running_elsewhere: bool,
}

/// Starts the game of `req.instance` as an operation and returns its id at once; the channel
/// behind `sink` gets `Preparing` first. The caller holds `AppState::instance_lock` and has just
/// loaded the instance. `Busy` while another game is being prepared or runs, `InstanceRunning`
/// while a game of the instance runs that this session does not know.
pub fn start(req: PlayRequest, sink: EventSink<PlayEvent>) -> Result<OperationId, ErrorDto> {
    let game = req.game.clone();
    let mut inner = lock(&game.inner);
    if !matches!(inner.phase, Phase::Idle { .. }) {
        return Err(busy());
    }
    if req.running_elsewhere {
        return Err(error_dto(&hengst_core::Error::InstanceRunning {
            id: req.instance.id.clone(),
        }));
    }
    let cancel = CancelFlag::new();
    let (out, _forwarder) = OpOut::<PlayEvent>::new(sink);
    let cell: Arc<OnceLock<OperationId>> = Arc::default();
    let on_cancel: Box<dyn Fn() + Send + Sync> = {
        let (game, out, cancel, cell) = (game.clone(), out.clone(), cancel.clone(), cell.clone());
        Box::new(move || {
            cancel.cancel();
            if let Some(op) = cell.get() {
                game.end_preparing(&mut lock(&game.inner), *op);
            }
            out.end(PlayEvent::Cancelled);
        })
    };
    let ops = req.ops.clone();
    // The table holds no `Play` operation while the phase is `Idle` (one leaves the table exactly
    // when the phase leaves `Preparing`), so `start` replaces nothing here and calls no
    // `on_cancel` that would need `inner`, which this thread holds.
    let op = ops.start(OpKind::Play, on_cancel, |op| {
        let _ = cell.set(op);
        // Queued before the task exists: always the first event.
        out.progress(PlayEvent::Preparing { op });
        game.set(
            &mut inner,
            Phase::Preparing {
                op,
                instance: req.instance.id.clone(),
            },
        );
        let task = PlayTask {
            op,
            out,
            cancel,
            req,
        };
        tokio::spawn(task.run()).abort_handle()
    });
    Ok(op)
}

struct PlayTask {
    op: OperationId,
    out: OpOut<PlayEvent>,
    cancel: CancelFlag,
    req: PlayRequest,
}

/// Held by the task of a start. If the task ends without deciding its terminal event (a panic),
/// the start still leaves the table, the state returns to `Idle` and the channel gets `Failed`.
/// After `cancel` removed the operation it does nothing.
struct PlayGuard {
    game: Arc<Game>,
    ops: Operations,
    op: OperationId,
    out: OpOut<PlayEvent>,
    armed: bool,
}

impl Drop for PlayGuard {
    fn drop(&mut self) {
        if self.armed {
            let mut inner = lock(&self.game.inner);
            if self.ops.finish(self.op) {
                self.game.end_preparing(&mut inner, self.op);
                self.out.end(PlayEvent::Failed {
                    error: internal("the game start ended without a result"),
                });
            }
        }
    }
}

impl PlayTask {
    async fn run(self) {
        let mut guard = PlayGuard {
            game: self.req.game.clone(),
            ops: self.req.ops.clone(),
            op: self.op,
            out: self.out.clone(),
            armed: true,
        };
        if let Err(error) = self.prepare_and_start().await {
            self.fail(error);
        }
        guard.armed = false;
    }

    /// Ends the start with `Failed`, if it still owns its end.
    fn fail(&self, error: ErrorDto) {
        let game = &self.req.game;
        let mut inner = lock(&game.inner);
        if self.req.ops.finish(self.op) {
            game.end_preparing(&mut inner, self.op);
            self.out.end(PlayEvent::Failed { error });
        }
    }

    async fn prepare_and_start(&self) -> Result<(), ErrorDto> {
        let r = &self.req;
        // Before anything else (the account step may renew over the network and write the
        // account store): the app's own memory value (an instance's comes from its own file, the
        // CLI's business) reaches the JVM arguments only from the range the settings allow, and
        // the Java path that would be started is checked again (text before any file-system
        // call, so a stored UNC path never connects); its canonical path is the one started.
        if r.instance.config.memory_mb.is_none()
            && !crate::commands::settings::MEMORY_MB.contains(&r.settings.memory_mb)
        {
            return Err(ErrorDto {
                action: Some(ErrorAction::OpenSettings),
                ..crate::commands::settings::invalid_input("memoryMb")
            });
        }
        let java = match r
            .instance
            .config
            .java
            .clone()
            .or_else(|| r.settings.java_override.clone())
        {
            Some(java) => {
                let checked = tokio::task::spawn_blocking(move || {
                    crate::commands::settings::checked_java(&java)
                })
                .await
                .map_err(|_| internal("a background task failed"))??;
                Some(checked)
            }
            None => None,
        };
        let (account, marked) = r.accounts.play_account(&r.settings).await;
        if marked {
            r.events.emit(AppEvent::DataChanged {
                scope: DataScope::Accounts,
            });
        }
        let (account, chosen) = account.map_err(|e| error_dto(&e))?;
        let mut opts = instances::launch_options(&r.instance, account, r.settings.memory_mb, None);
        opts.java = java;
        let progress: ProgressSink = {
            let (out, root) = (self.out.clone(), r.paths.root().to_path_buf());
            Arc::new(move |p| {
                if let Some(event) = play_event(p, &root) {
                    out.progress(event);
                }
            })
        };
        let preparer = lock(&r.game.preparer).clone();
        let prepared = preparer(PrepareJob {
            paths: r.paths.clone(),
            opts,
            endpoints: r.endpoints.clone(),
            cancel: self.cancel.clone(),
            progress,
        })
        .await
        .map_err(|e| play_error(&e, r.paths.root()))?;
        // Signed out (or flagged) during the preparation: the token in memory is not used.
        if let Some(chosen) = &chosen {
            r.accounts
                .still_present(chosen)
                .map_err(|e| error_dto(&e))?;
        }
        // From here on the start cannot be cancelled: an abort during the spawn would leave a
        // running game nobody watches.
        if self.cancel.is_cancelled() || !r.ops.try_commit(self.op) {
            return Ok(());
        }
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<LogLine>();
        let sink: LineSink = Arc::new(move |line| {
            let _ = tx.send(line);
        });
        let process = spawn_game(&prepared, sink)
            .await
            .map_err(|e| error_dto(&e))?;
        self.running(Arc::new(process), rx);
        record_launch(r, &prepared).await;
        Ok(())
    }

    /// The process runs: `Running`, `Started` to the channel, the watcher and the line drain.
    fn running(
        &self,
        process: Arc<GameProcess>,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<LogLine>,
    ) {
        let game = self.req.game.clone();
        let run_id = {
            let mut inner = lock(&game.inner);
            // Committed, so `cancel` cannot have removed it: this always succeeds.
            self.req.ops.finish(self.op);
            inner.last_run += 1;
            let run_id = RunId(inner.last_run);
            let since = unix_millis();
            // Before `Running` is announced, so an attach that follows the state finds the run.
            game.log.start_run(
                run_id,
                &self.req.instance.id,
                since,
                process.log_path(),
                process.utc_offset(),
            );
            let run = Run {
                instance: self.req.instance.id.clone(),
                run_id,
                since,
                process: process.clone(),
                stops_in_flight: 0,
                asked: false,
                forcing: 0,
                forced: false,
            };
            game.set(&mut inner, Phase::Running(run));
            self.out.end(PlayEvent::Started { run_id });
            run_id
        };
        let drain = game.clone();
        tokio::spawn(async move {
            // The channel closes when the game's output ends: the run's log is complete.
            let _closer = RunCloser {
                log: drain.log.clone(),
                run_id,
            };
            while let Some(line) = rx.recv().await {
                if let Some(handler) = lock(&drain.lines).clone() {
                    let copy = line.clone();
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        handler(run_id, copy)
                    }));
                }
                drain.log.push(run_id, line);
            }
        });
        tokio::spawn(async move {
            let mut guard = RunGuard {
                game: game.clone(),
                run_id,
                armed: true,
            };
            let code = process.wait().await.unwrap_or(-1);
            guard.armed = false;
            game.ended(run_id, code);
        });
    }
}

/// Held by the task that waits for a run's exit: if it ends without one (a panic, the runtime
/// shutting down), the run still ends in the state, with code -1.
struct RunGuard {
    game: Arc<Game>,
    run_id: RunId,
    armed: bool,
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        if self.armed {
            self.game.ended(self.run_id, -1);
        }
    }
}

/// `last_played` and, for an instance whose loader version is still open, the resolved one.
/// Only after a successful start; a failure here does not touch the running game.
async fn record_launch(r: &PlayRequest, prepared: &Prepared) {
    let (paths, id, loader) = (
        r.paths.clone(),
        r.instance.id.clone(),
        prepared.loader.clone(),
    );
    let saved = tokio::task::spawn_blocking(move || {
        instances::record_launch(&paths, &id, unix_now(), loader.as_ref())
    })
    .await;
    match saved {
        Ok(Ok(())) => r.events.emit(AppEvent::DataChanged {
            scope: DataScope::Instances,
        }),
        Ok(Err(e)) => eprintln!(
            "play: could not record the launch of {}: {}",
            r.instance.id,
            strip_paths(&e.to_string())
        ),
        Err(_) => eprintln!("play: the task recording the launch failed"),
    }
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Asks the running game to close (its windows get the close request). See `StopResult`.
pub async fn stop(game: &Arc<Game>) -> Result<StopResult, ErrorDto> {
    let Some((process, run_id)) = begin_stop(game) else {
        return Ok(StopResult::NotRunning);
    };
    let posted = tokio::task::spawn_blocking(move || process.request_stop())
        .await
        .map_err(|_| internal("the task that stops the game failed"))
        .and_then(|r| r.map_err(|e| error_dto(&e)));
    finish_stop(game, run_id, posted)
}

/// Applies `change` to the run `run_id` if it still runs, and tells the web view when its state
/// shows differently afterwards. `None`: that run is over.
fn change_run<T>(game: &Game, run_id: RunId, change: impl FnOnce(&mut Run) -> T) -> Option<T> {
    let mut inner = lock(&game.inner);
    let before = dto(&inner.phase);
    let Phase::Running(run) = &mut inner.phase else {
        return None;
    };
    if run.run_id != run_id {
        return None;
    }
    let out = change(run);
    if dto(&inner.phase) != before {
        game.emit(&inner);
    }
    Some(out)
}

fn running_id(game: &Game) -> Option<RunId> {
    match &lock(&game.inner).phase {
        Phase::Running(run) => Some(run.run_id),
        _ => None,
    }
}

/// A stop request goes out: counted while in flight.
fn begin_stop(game: &Game) -> Option<(Arc<GameProcess>, RunId)> {
    let run_id = running_id(game)?;
    change_run(game, run_id, |run| {
        run.stops_in_flight += 1;
        (run.process.clone(), run_id)
    })
}

fn finish_stop(
    game: &Game,
    run_id: RunId,
    posted: Result<usize, ErrorDto>,
) -> Result<StopResult, ErrorDto> {
    let reached = matches!(posted, Ok(n) if n > 0);
    let still = change_run(game, run_id, |run| {
        run.stops_in_flight -= 1;
        run.asked |= reached;
    });
    match (posted, still) {
        (Err(e), _) => Err(e),
        (Ok(_), _) if reached => Ok(StopResult::Asked),
        (Ok(_), Some(())) => Ok(StopResult::NoWindow),
        // The run ended meanwhile.
        (Ok(_), None) => Ok(StopResult::NotRunning),
    }
}

/// Ends every process of the running game at once. `false` when no game runs.
pub async fn force_stop(game: &Arc<Game>) -> Result<bool, ErrorDto> {
    let Some((process, run_id)) = begin_force(game) else {
        return Ok(false);
    };
    let result = tokio::task::spawn_blocking(move || process.force_stop())
        .await
        .map_err(|_| internal("the task that stops the game failed"))
        .and_then(|r| r.map_err(|e| error_dto(&e)));
    finish_force(game, run_id, result)
}

/// A force stop starts: `Stopping` and requested while it runs.
fn begin_force(game: &Game) -> Option<(Arc<GameProcess>, RunId)> {
    let run_id = running_id(game)?;
    change_run(game, run_id, |run| {
        run.forcing += 1;
        (run.process.clone(), run_id)
    })
}

/// A failed termination takes its mark back: the game runs on as before.
fn finish_force(
    game: &Game,
    run_id: RunId,
    result: Result<(), ErrorDto>,
) -> Result<bool, ErrorDto> {
    let ok = result.is_ok();
    change_run(game, run_id, |run| {
        run.forcing -= 1;
        run.forced |= ok;
    });
    result.map(|()| true)
}

/// Loads the instance and looks at its run lock under the instance lock, then starts it (see
/// `start`). The run lock is checked here only to refuse early and clearly: the core takes it
/// when the process starts and refuses there too.
pub async fn play(
    state: &crate::state::AppState,
    instance_id: String,
    sink: EventSink<PlayEvent>,
) -> Result<OperationId, ErrorDto> {
    let _instances = state.instance_lock.lock().await;
    let (paths, id) = (state.paths.clone(), instance_id.clone());
    let (instance, running_elsewhere) = tokio::task::spawn_blocking(move || {
        let instance = instances::load(&paths, &id)?;
        let held = hengst_core::runlock::is_running(&paths, &id)?;
        Ok::<_, hengst_core::Error>((instance, held))
    })
    .await
    .map_err(|_| internal("a background task failed"))?
    .map_err(|e| instance_id_error(&e, &instance_id))?;
    start(
        PlayRequest {
            game: state.game.clone(),
            ops: state.operations.clone(),
            events: state.events.clone(),
            accounts: state.accounts.clone(),
            paths: state.paths.clone(),
            endpoints: state.endpoints.clone(),
            settings: state.settings.get(),
            instance,
            running_elsewhere,
        },
        sink,
    )
}

#[cfg(test)]
#[path = "play_tests.rs"]
mod tests;
