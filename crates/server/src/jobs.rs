//! Job queue with one worker, cancellation, change events and persistence.
//!
//! Jobs run one at a time (each job already translates its chunks
//! concurrently). Every state or phase change bumps the job's revision, is
//! written to `<home>/jobs/<id>.json` and is broadcast to event subscribers;
//! progress counts only nudge subscribers (they read the current value) and
//! are written to disk at most every [`PROGRESS_PERSIST_INTERVAL_MS`].
//!
//! Jobs that were queued or running when the server stopped come back as
//! failed with [`error_codes::INTERRUPTED`]: the API key a job needs is never
//! persisted, so it cannot be resumed safely.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

use animesubs_api::{
    ApiError, JobSpec, JobState, JobStatus, PersistedJob, ProviderConfig, TranslateSubtitleJob,
    error_codes,
};
use tokio::sync::{broadcast, mpsc, watch};

use crate::auth::random_id;
use crate::control::{CancelOutcome, JobControl};
use crate::now_ms;
use crate::ops::{self, TranslatePlan};
use crate::progress::{JobProgress, ProgressChange};
use crate::store::JobStore;

/// Finished (succeeded, failed or cancelled) jobs kept in memory and on disk.
pub const MAX_FINISHED_JOBS: usize = 200;
/// Minimum time between two progress-only writes of a job file.
pub const PROGRESS_PERSIST_INTERVAL_MS: u64 = 2_000;
/// Transitions buffered per event subscriber before it lags (and resyncs).
const EVENT_CAPACITY: usize = 256;

/// A job status with its revision: higher revisions are newer state or phase
/// changes; progress counts change without a new revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobEvent {
    pub rev: u64,
    pub status: JobStatus,
}

/// Why a job could not be cancelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelError {
    NotFound,
    /// Already succeeded or failed, or its output is already in place.
    NotCancellable(JobState),
}

struct Record {
    status: JobStatus,
    progress: Arc<JobProgress>,
    /// Taken by the worker; dropping it after the run also drops the API key.
    plan: Option<TranslatePlan>,
    /// What is persisted: the request without secrets.
    spec: JobSpec,
    control: Arc<JobControl>,
    rev: u64,
    persisted_ms: u64,
}

#[derive(Default)]
struct Table {
    order: Vec<String>,
    records: HashMap<String, Record>,
    seq: u64,
    /// The server is stopping: running jobs are abandoned and files left as they are.
    halted: bool,
}

struct Shared {
    table: Mutex<Table>,
    events: broadcast::Sender<JobEvent>,
    ticks: watch::Sender<u64>,
    store: Option<JobStore>,
}

/// Cloneable handle to the queue.
#[derive(Clone)]
pub struct JobQueue {
    shared: Arc<Shared>,
    sender: mpsc::UnboundedSender<String>,
}

impl JobQueue {
    /// Create the queue, load the jobs persisted in `store` and spawn the
    /// worker on the current Tokio runtime.
    pub fn start(store: Option<JobStore>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (ticks, _) = watch::channel(0);
        let queue = Self {
            shared: Arc::new(Shared {
                table: Mutex::default(),
                events,
                ticks,
                store,
            }),
            sender,
        };
        queue.shared.restore();
        tokio::spawn(queue.clone().work(receiver));
        queue
    }

    pub fn submit(&self, plan: TranslatePlan) -> JobStatus {
        let id = format!("job-{}", random_id(8));
        let status = JobStatus {
            id: id.clone(),
            kind: "translate_subtitle".into(),
            state: JobState::Queued,
            phase: "queued".into(),
            progress: None,
            input: plan.input.clone(),
            output: Some(plan.output.clone()),
            target_language: plan.target_language.clone(),
            created_at_ms: now_ms(),
            started_at_ms: None,
            finished_at_ms: None,
            error: None,
        };
        let progress = Arc::new(JobProgress::new("queued"));
        progress.set_listener(listener(Arc::downgrade(&self.shared), id.clone()));
        {
            let mut table = self.shared.lock();
            table.order.push(id.clone());
            table.records.insert(
                id.clone(),
                Record {
                    status: status.clone(),
                    progress,
                    spec: spec_without_secrets(&plan),
                    plan: Some(plan),
                    control: JobControl::new(),
                    rev: 0,
                    persisted_ms: 0,
                },
            );
            self.shared.publish(&mut table, &id);
        }
        // The worker lives as long as the runtime; a send error means shutdown.
        let _ = self.sender.send(id);
        status
    }

    pub fn get(&self, id: &str) -> Option<JobStatus> {
        self.shared.lock().records.get(id).map(snapshot)
    }

    /// All jobs, oldest first.
    pub fn list(&self) -> Vec<JobStatus> {
        self.events()
            .into_iter()
            .map(|event| event.status)
            .collect()
    }

    /// All jobs with their revisions, oldest first.
    pub fn events(&self) -> Vec<JobEvent> {
        events_of(&self.shared.lock(), |_| true)
    }

    /// Queued and running jobs with their revisions, oldest first.
    pub fn active_events(&self) -> Vec<JobEvent> {
        events_of(&self.shared.lock(), |record| {
            !record.status.state.is_terminal()
        })
    }

    /// Current jobs plus every later transition (and a nudge on progress
    /// changes), with nothing lost between the two.
    pub fn subscribe(
        &self,
    ) -> (
        Vec<JobEvent>,
        broadcast::Receiver<JobEvent>,
        watch::Receiver<u64>,
    ) {
        let table = self.shared.lock();
        let transitions = self.shared.events.subscribe();
        let ticks = self.shared.ticks.subscribe();
        (events_of(&table, |_| true), transitions, ticks)
    }

    /// Jobs queued or running.
    pub fn active(&self) -> usize {
        self.shared
            .lock()
            .records
            .values()
            .filter(|record| !record.status.state.is_terminal())
            .count()
    }

    /// Cancel a queued or running job. Cancelling a cancelled job returns it unchanged.
    pub fn cancel(&self, id: &str) -> Result<JobStatus, CancelError> {
        let mut table = self.shared.lock();
        let record = table.records.get_mut(id).ok_or(CancelError::NotFound)?;
        match record.status.state {
            JobState::Cancelled => return Ok(snapshot(record)),
            state @ (JobState::Succeeded | JobState::Failed) => {
                return Err(CancelError::NotCancellable(state));
            }
            JobState::Queued => {
                record.plan = None;
                record.control.cancel();
            }
            JobState::Running => {
                if record.control.cancel() == CancelOutcome::TooLate {
                    return Err(CancelError::NotCancellable(JobState::Running));
                }
            }
        }
        record.status.state = JobState::Cancelled;
        record.status.finished_at_ms = Some(now_ms());
        record.status.error = None;
        record.progress.set_phase_silently("cancelled");
        self.shared.publish(&mut table, id);
        let status = table.records.get(id).map(snapshot);
        self.shared.prune(&mut table);
        status.ok_or(CancelError::NotFound)
    }

    /// The server is stopping: abort the running job and stop recording
    /// changes, so its file still says it was running (and it is reported as
    /// interrupted after a restart).
    pub fn halt(&self) {
        let mut table = self.shared.lock();
        table.halted = true;
        for record in table.records.values() {
            if record.status.state == JobState::Running {
                record.control.abort_now();
            }
        }
    }

    async fn work(self, mut receiver: mpsc::UnboundedReceiver<String>) {
        while let Some(id) = receiver.recv().await {
            let Some((plan, progress, control)) = self.begin(&id) else {
                if self.shared.lock().halted {
                    return;
                }
                continue;
            };
            // Run on its own task so a panic in the engine fails the job, not
            // the worker, and so cancelling can abort it.
            let task = tokio::spawn({
                let control = control.clone();
                async move { ops::run_translate_with(&plan, progress, &control).await }
            });
            control.set_abort(task.abort_handle());
            let outcome = match task.await {
                Ok(result) => result.map_err(|error| error.to_api()),
                Err(error) if error.is_cancelled() => Err(ApiError {
                    code: ops::CANCELLED.into(),
                    message: "job was cancelled".into(),
                }),
                Err(error) => Err(ApiError {
                    code: error_codes::INTERNAL.into(),
                    message: format!("job task failed: {error}"),
                }),
            };
            self.finish(&id, outcome);
        }
    }

    fn begin(&self, id: &str) -> Option<(TranslatePlan, Arc<JobProgress>, Arc<JobControl>)> {
        let mut table = self.shared.lock();
        if table.halted {
            return None;
        }
        let record = table.records.get_mut(id)?;
        let plan = record.plan.take()?;
        record.status.state = JobState::Running;
        record.status.started_at_ms = Some(now_ms());
        record.progress.set_phase_silently("starting");
        let started = (plan, record.progress.clone(), record.control.clone());
        self.shared.publish(&mut table, id);
        Some(started)
    }

    fn finish(&self, id: &str, outcome: Result<std::path::PathBuf, ApiError>) {
        let mut table = self.shared.lock();
        if table.halted {
            return;
        }
        let Some(record) = table.records.get_mut(id) else {
            return;
        };
        // Cancelled jobs were already finished by `cancel`.
        if record.status.state.is_terminal() {
            return;
        }
        record.status.finished_at_ms = Some(now_ms());
        match outcome {
            Ok(output) => {
                record.status.state = JobState::Succeeded;
                record.status.output = Some(output);
            }
            Err(error) => {
                record.status.state = JobState::Failed;
                record.progress.set_phase_silently("failed");
                record.status.error = Some(error);
            }
        }
        self.shared.publish(&mut table, id);
        self.shared.prune(&mut table);
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Table> {
        // A panic while holding the lock leaves plain data behind; keep serving it.
        self.table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Record a state or phase change of `id`: new revision, file, broadcast.
    fn publish(&self, table: &mut Table, id: &str) {
        if table.halted {
            return;
        }
        table.seq += 1;
        let rev = table.seq;
        let Some(record) = table.records.get_mut(id) else {
            return;
        };
        record.rev = rev;
        let status = snapshot(record);
        self.persist(record, &status);
        // No subscribers is fine.
        let _ = self.events.send(JobEvent { rev, status });
    }

    fn progress_changed(&self, id: &str) {
        self.ticks
            .send_modify(|ticks| *ticks = ticks.wrapping_add(1));
        let mut table = self.lock();
        if table.halted {
            return;
        }
        if let Some(record) = table.records.get_mut(id)
            && !record.status.state.is_terminal()
            && now_ms().saturating_sub(record.persisted_ms) >= PROGRESS_PERSIST_INTERVAL_MS
        {
            let status = snapshot(record);
            self.persist(record, &status);
        }
    }

    fn persist(&self, record: &mut Record, status: &JobStatus) {
        let Some(store) = &self.store else {
            return;
        };
        let job = PersistedJob {
            status: status.clone(),
            spec: record.spec.clone(),
        };
        if let Err(error) = store.save(&job) {
            eprintln!("[jobs] could not save {}: {error}", status.id);
        }
        record.persisted_ms = now_ms();
    }

    /// Load persisted jobs; those that did not finish become `interrupted` failures.
    fn restore(&self) {
        let Some(store) = &self.store else {
            return;
        };
        let mut jobs = store.load();
        jobs.sort_by(|a, b| {
            (a.status.created_at_ms, &a.status.id).cmp(&(b.status.created_at_ms, &b.status.id))
        });
        let mut table = self.lock();
        for PersistedJob { mut status, spec } in jobs {
            let interrupted = !status.state.is_terminal();
            if interrupted {
                status.state = JobState::Failed;
                status.phase = "failed".into();
                status.finished_at_ms = Some(now_ms());
                status.error = Some(ApiError {
                    code: error_codes::INTERRUPTED.into(),
                    message: "the server stopped before this job finished; submit it again".into(),
                });
            }
            let progress = JobProgress::new(&status.phase);
            if let Some(counts) = status.progress {
                progress.set_total(counts.total);
                progress.add_done(counts.done);
            }
            let id = status.id.clone();
            let mut record = Record {
                status,
                progress: Arc::new(progress),
                plan: None,
                spec,
                control: JobControl::new(),
                rev: 0,
                persisted_ms: 0,
            };
            if interrupted {
                let status = snapshot(&record);
                self.persist(&mut record, &status);
            }
            if table.records.insert(id.clone(), record).is_none() {
                table.order.push(id);
            }
        }
        self.prune(&mut table);
    }

    /// Forget the oldest finished jobs beyond [`MAX_FINISHED_JOBS`].
    fn prune(&self, table: &mut Table) {
        let finished: Vec<&String> = table
            .order
            .iter()
            .filter(|id| {
                table
                    .records
                    .get(*id)
                    .is_some_and(|record| record.status.state.is_terminal())
            })
            .collect();
        let Some(excess) = finished.len().checked_sub(MAX_FINISHED_JOBS) else {
            return;
        };
        let doomed: HashSet<String> = finished[..excess].iter().map(|id| (*id).clone()).collect();
        table.order.retain(|id| !doomed.contains(id));
        for id in &doomed {
            table.records.remove(id);
            if let Some(store) = &self.store {
                store.remove(id);
            }
        }
    }
}

fn listener(shared: Weak<Shared>, id: String) -> impl Fn(ProgressChange) + Send + Sync + 'static {
    move |change| {
        let Some(shared) = shared.upgrade() else {
            return;
        };
        match change {
            ProgressChange::Phase => {
                let mut table = shared.lock();
                shared.publish(&mut table, &id);
            }
            ProgressChange::Count => shared.progress_changed(&id),
        }
    }
}

fn events_of(table: &Table, keep: impl Fn(&Record) -> bool) -> Vec<JobEvent> {
    table
        .order
        .iter()
        .filter_map(|id| table.records.get(id))
        .filter(|record| keep(record))
        .map(|record| JobEvent {
            rev: record.rev,
            status: snapshot(record),
        })
        .collect()
}

fn snapshot(record: &Record) -> JobStatus {
    let mut status = record.status.clone();
    status.phase = record.progress.phase();
    status.progress = record.progress.snapshot();
    status
}

fn spec_without_secrets(plan: &TranslatePlan) -> JobSpec {
    JobSpec::TranslateSubtitle(TranslateSubtitleJob {
        input: plan.input.clone(),
        output: Some(plan.output.clone()),
        target_language: plan.target_language.clone(),
        provider: ProviderConfig {
            api_key: None,
            ..plan.provider.clone()
        },
        parallel: Some(plan.parallel),
        overwrite: plan.overwrite,
    })
}
