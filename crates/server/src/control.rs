//! Cancellation of a running job.
//!
//! A job is cancelled by aborting its task, except while it writes its
//! output: aborting then could leave the temporary file behind (blocking file
//! I/O runs to completion even when the future awaiting it is dropped), so the
//! writer checks for cancellation itself, removes the temporary file and only
//! renames it into place while no cancellation has been requested. Once the
//! output is in place the job can no longer be cancelled.

use std::sync::{Arc, Mutex};

use tokio::task::AbortHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Running,
    Writing,
    Committed,
    Cancelled,
}

#[derive(Debug)]
struct State {
    stage: Stage,
    abort: Option<AbortHandle>,
    /// Cancelled while running but before [`JobControl::set_abort`].
    abort_pending: bool,
}

/// Result of [`JobControl::cancel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    /// The job will stop without writing its output.
    Cancelled,
    /// It was already cancelled.
    AlreadyCancelled,
    /// Its output is already in place.
    TooLate,
}

/// Shared between a job's task, the worker and the cancel endpoint.
#[derive(Debug)]
pub struct JobControl {
    state: Mutex<State>,
}

impl Default for JobControl {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                stage: Stage::Running,
                abort: None,
                abort_pending: false,
            }),
        }
    }
}

impl JobControl {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Request cancellation: aborts the task unless it is writing its output.
    pub fn cancel(&self) -> CancelOutcome {
        let mut state = self.lock();
        match state.stage {
            Stage::Cancelled => CancelOutcome::AlreadyCancelled,
            Stage::Committed => CancelOutcome::TooLate,
            Stage::Writing => {
                state.stage = Stage::Cancelled;
                CancelOutcome::Cancelled
            }
            Stage::Running => {
                state.stage = Stage::Cancelled;
                match state.abort.take() {
                    Some(abort) => abort.abort(),
                    None => state.abort_pending = true,
                }
                CancelOutcome::Cancelled
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.lock().stage == Stage::Cancelled
    }

    /// Register the task running the job.
    pub fn set_abort(&self, abort: AbortHandle) {
        let mut state = self.lock();
        if state.abort_pending {
            abort.abort();
        } else {
            state.abort = Some(abort);
        }
    }

    /// Abort the task without marking the job cancelled (server shutdown).
    pub(crate) fn abort_now(&self) {
        if let Some(abort) = self.lock().abort.take() {
            abort.abort();
        }
    }

    /// The task is about to write its output; from now on it is not aborted.
    /// `false` if the job was cancelled.
    pub fn begin_writing(&self) -> bool {
        let mut state = self.lock();
        if state.stage == Stage::Cancelled {
            return false;
        }
        state.stage = Stage::Writing;
        state.abort = None;
        true
    }

    /// Run `put_in_place` unless the job was cancelled; after it the job is
    /// no longer cancellable. `None` if cancelled.
    pub fn commit<T>(&self, put_in_place: impl FnOnce() -> T) -> Option<T> {
        let mut state = self.lock();
        if state.stage == Stage::Cancelled {
            return None;
        }
        let result = put_in_place();
        state.stage = Stage::Committed;
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages() {
        let control = JobControl::new();
        assert!(control.begin_writing());
        assert_eq!(control.commit(|| 7), Some(7));
        assert_eq!(control.cancel(), CancelOutcome::TooLate);

        let control = JobControl::new();
        assert!(control.begin_writing());
        assert_eq!(control.cancel(), CancelOutcome::Cancelled);
        assert_eq!(control.commit(|| 7), None);
        assert_eq!(control.cancel(), CancelOutcome::AlreadyCancelled);

        let control = JobControl::new();
        assert_eq!(control.cancel(), CancelOutcome::Cancelled);
        assert!(control.is_cancelled());
        assert!(!control.begin_writing());
    }

    #[tokio::test]
    async fn cancel_before_the_task_is_registered_still_aborts_it() {
        let control = JobControl::new();
        control.cancel();
        let task = tokio::spawn(std::future::pending::<()>());
        control.set_abort(task.abort_handle());
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
