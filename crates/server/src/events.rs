//! `GET /v1/events`: job updates as Server-Sent Events.
//!
//! A subscriber first gets one frame per existing job (oldest first), then a
//! frame for every state or phase change and, at most every
//! [`PROGRESS_INTERVAL`] per job, for progress changes. A subscriber that
//! falls behind skips intermediate frames but always ends up with each job's
//! latest status, so it never misses a terminal frame.

use std::collections::HashMap;
use std::time::Duration;

use animesubs_api::{JOB_EVENT, JobStatus, Progress};
use axum::body::Bytes;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

use crate::jobs::{JobEvent, JobQueue};

/// A `: keep-alive` comment is sent this often.
pub const KEEP_ALIVE: Duration = Duration::from_secs(15);
/// Minimum time between two progress-only frames of one job.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

const KEEP_ALIVE_FRAME: &str = ": keep-alive\n\n";

/// One SSE frame: `event: job` and the status as JSON on one `data:` line.
pub fn frame(status: &JobStatus) -> Bytes {
    // Serializing plain data cannot fail; JSON never contains a raw newline.
    let json = serde_json::to_string(status).unwrap_or_default();
    Bytes::from(format!("event: {JOB_EVENT}\ndata: {json}\n\n"))
}

/// `job` query parameter of `/v1/events?job=<id>`.
pub fn job_filter(query: Option<&str>) -> Option<String> {
    query?
        .split('&')
        .find_map(|pair| pair.strip_prefix("job="))
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

struct Sent {
    rev: u64,
    progress: Option<Progress>,
    at: Instant,
}

/// What one subscriber has been sent, to decide what to send next.
struct Subscriber {
    filter: Option<String>,
    sent: HashMap<String, Sent>,
    /// When a coalesced progress frame is due.
    flush_at: Option<Instant>,
    out: Vec<Bytes>,
}

impl Subscriber {
    fn new(filter: Option<String>) -> Self {
        Self {
            filter,
            sent: HashMap::new(),
            flush_at: None,
            out: Vec::new(),
        }
    }

    fn offer(&mut self, events: impl IntoIterator<Item = JobEvent>) {
        let now = Instant::now();
        for event in events {
            let id = &event.status.id;
            if self.filter.as_ref().is_some_and(|wanted| wanted != id) {
                continue;
            }
            if let Some(sent) = self.sent.get(id) {
                if event.rev < sent.rev {
                    continue;
                }
                if event.rev == sent.rev {
                    if event.status.progress == sent.progress {
                        continue;
                    }
                    let due = sent.at + PROGRESS_INTERVAL;
                    if now < due {
                        self.flush_at = Some(self.flush_at.map_or(due, |at| at.min(due)));
                        continue;
                    }
                }
            }
            self.out.push(frame(&event.status));
            self.sent.insert(
                id.clone(),
                Sent {
                    rev: event.rev,
                    progress: event.status.progress,
                    at: now,
                },
            );
        }
    }
}

/// Feed `tx` until the client goes away or the server stops. `guard` lives as
/// long as the stream (it marks a client as attached).
pub async fn run<G: Send + 'static>(
    queue: JobQueue,
    filter: Option<String>,
    mut stopping: watch::Receiver<bool>,
    tx: mpsc::Sender<Bytes>,
    guard: G,
) {
    let _guard = guard;
    let (snapshot, mut transitions, mut ticks) = queue.subscribe();
    let mut subscriber = Subscriber::new(filter);
    subscriber.offer(snapshot);
    let mut keep_alive = tokio::time::interval_at(Instant::now() + KEEP_ALIVE, KEEP_ALIVE);
    loop {
        for frame in std::mem::take(&mut subscriber.out) {
            tokio::select! {
                biased;
                _ = stopped(&mut stopping) => return,
                sent = tx.send(frame) => if sent.is_err() { return },
            }
        }
        let flush_at = subscriber.flush_at;
        tokio::select! {
            biased;
            _ = stopped(&mut stopping) => return,
            _ = tx.closed() => return,
            received = transitions.recv() => match received {
                Ok(event) => subscriber.offer([event]),
                // Fell behind: send the latest status of every job instead.
                Err(RecvError::Lagged(_)) => subscriber.offer(queue.events()),
                Err(RecvError::Closed) => return,
            },
            changed = ticks.changed() => {
                if changed.is_err() {
                    return;
                }
                subscriber.offer(queue.active_events());
            }
            _ = tokio::time::sleep_until(flush_at.unwrap_or_else(Instant::now)),
                if flush_at.is_some() =>
            {
                subscriber.flush_at = None;
                subscriber.offer(queue.active_events());
            }
            _ = keep_alive.tick() => subscriber.out.push(Bytes::from_static(KEEP_ALIVE_FRAME.as_bytes())),
        }
    }
}

async fn stopped(stopping: &mut watch::Receiver<bool>) {
    // A dropped sender means the server state is gone: stop too.
    let _ = stopping.wait_for(|stop| *stop).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use animesubs_api::JobState;

    fn event(id: &str, rev: u64, done: Option<u64>) -> JobEvent {
        JobEvent {
            rev,
            status: JobStatus {
                id: id.into(),
                kind: "translate_subtitle".into(),
                state: JobState::Running,
                phase: "translating".into(),
                progress: done.map(|done| Progress { done, total: 10 }),
                input: "/m/a.srt".into(),
                output: None,
                target_language: "pt-BR".into(),
                created_at_ms: 0,
                started_at_ms: None,
                finished_at_ms: None,
                error: None,
            },
        }
    }

    #[test]
    fn frames_are_sse_job_events() {
        let frame = frame(&event("job-1", 1, None).status);
        let text = std::str::from_utf8(&frame).unwrap();
        assert!(
            text.starts_with("event: job\ndata: {\"id\":\"job-1\""),
            "{text}"
        );
        assert!(text.ends_with("}\n\n"));
        assert_eq!(text.matches('\n').count(), 3);
    }

    #[test]
    fn query_filter() {
        assert_eq!(job_filter(Some("job=job-1")), Some("job-1".into()));
        assert_eq!(job_filter(Some("x=1&job=job-2")), Some("job-2".into()));
        assert_eq!(job_filter(Some("job=")), None);
        assert_eq!(job_filter(None), None);
    }

    #[test]
    fn transitions_pass_progress_is_coalesced_old_revisions_dropped() {
        let mut subscriber = Subscriber::new(None);
        subscriber.offer([event("a", 1, None), event("b", 1, None)]);
        assert_eq!(subscriber.out.len(), 2);
        subscriber.out.clear();

        // Progress right after a frame waits for the interval.
        subscriber.offer([event("a", 1, Some(1))]);
        assert!(subscriber.out.is_empty());
        assert!(subscriber.flush_at.is_some());
        // A transition never waits; an older revision is never sent.
        subscriber.offer([event("a", 2, Some(2)), event("b", 0, Some(5))]);
        assert_eq!(subscriber.out.len(), 1);
        subscriber.out.clear();

        std::thread::sleep(PROGRESS_INTERVAL);
        subscriber.offer([event("a", 2, Some(3)), event("a", 2, Some(3))]);
        assert_eq!(subscriber.out.len(), 1);
    }

    #[test]
    fn filter_keeps_one_job() {
        let mut subscriber = Subscriber::new(Some("b".into()));
        subscriber.offer([event("a", 1, None), event("b", 1, None)]);
        assert_eq!(subscriber.out.len(), 1);
    }
}
