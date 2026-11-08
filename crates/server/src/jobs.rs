//! In-memory job queue with one worker.
//!
//! Jobs run one at a time (each job already translates its chunks
//! concurrently). The queue is in memory for now: jobs do not survive a
//! server restart. Persisting them is part of the next step in
//! `docs/ARCHITECTURE.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use animesubs_api::{JobState, JobStatus};
use tokio::sync::mpsc;

use crate::auth::random_id;
use crate::now_ms;
use crate::ops::{self, TranslatePlan};
use crate::progress::JobProgress;

struct Record {
    status: JobStatus,
    progress: Arc<JobProgress>,
    /// Taken by the worker; dropping it after the run also drops the API key.
    plan: Option<TranslatePlan>,
}

#[derive(Default)]
struct Table {
    order: Vec<String>,
    records: HashMap<String, Record>,
}

/// Cloneable handle to the queue.
#[derive(Clone)]
pub struct JobQueue {
    table: Arc<Mutex<Table>>,
    sender: mpsc::UnboundedSender<String>,
}

impl JobQueue {
    /// Create the queue and spawn its worker on the current Tokio runtime.
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queue = Self {
            table: Arc::default(),
            sender,
        };
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
        {
            let mut table = self.lock();
            table.order.push(id.clone());
            table.records.insert(
                id.clone(),
                Record {
                    status: status.clone(),
                    progress: Arc::new(JobProgress::new("queued")),
                    plan: Some(plan),
                },
            );
        }
        // The worker lives as long as the runtime; a send error means shutdown.
        let _ = self.sender.send(id);
        status
    }

    pub fn get(&self, id: &str) -> Option<JobStatus> {
        self.lock().records.get(id).map(snapshot)
    }

    /// All jobs, oldest first.
    pub fn list(&self) -> Vec<JobStatus> {
        let table = self.lock();
        table
            .order
            .iter()
            .filter_map(|id| table.records.get(id))
            .map(snapshot)
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Table> {
        // A panic while holding the lock leaves plain data behind; keep serving it.
        self.table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    async fn work(self, mut receiver: mpsc::UnboundedReceiver<String>) {
        while let Some(id) = receiver.recv().await {
            let Some((plan, progress)) = self.begin(&id) else {
                continue;
            };
            // Run on its own task so a panic in the engine fails the job, not the worker.
            let task = tokio::spawn({
                let progress = progress.clone();
                async move { ops::run_translate(&plan, progress).await }
            });
            let outcome = match task.await {
                Ok(result) => result.map_err(|error| error.to_api()),
                Err(error) => Err(animesubs_api::ApiError {
                    code: animesubs_api::error_codes::INTERNAL.into(),
                    message: format!("job task failed: {error}"),
                }),
            };
            self.finish(&id, outcome);
        }
    }

    fn begin(&self, id: &str) -> Option<(TranslatePlan, Arc<JobProgress>)> {
        let mut table = self.lock();
        let record = table.records.get_mut(id)?;
        let plan = record.plan.take()?;
        record.status.state = JobState::Running;
        record.status.started_at_ms = Some(now_ms());
        record.progress.set_phase("starting");
        Some((plan, record.progress.clone()))
    }

    fn finish(&self, id: &str, outcome: Result<std::path::PathBuf, animesubs_api::ApiError>) {
        let mut table = self.lock();
        let Some(record) = table.records.get_mut(id) else {
            return;
        };
        record.status.finished_at_ms = Some(now_ms());
        match outcome {
            Ok(output) => {
                record.status.state = JobState::Succeeded;
                record.status.output = Some(output);
            }
            Err(error) => {
                record.status.state = JobState::Failed;
                record.progress.set_phase("failed");
                record.status.error = Some(error);
            }
        }
    }
}

fn snapshot(record: &Record) -> JobStatus {
    let mut status = record.status.clone();
    status.phase = record.progress.phase();
    status.progress = record.progress.snapshot();
    status
}
