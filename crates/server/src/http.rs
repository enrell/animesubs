//! HTTP routes (API v1).

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use animesubs_api::{
    API_VERSION, ApiError, ErrorBody, Health, JobList, JobSpec, JobState, JobStatus, SERVICE_NAME,
    TracksRequest, TracksResponse, error_codes,
};
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::{Notify, mpsc, watch};

use crate::jobs::{CancelError, JobQueue};
use crate::ops::{self, OpError};
use crate::store::JobStore;
use crate::{VERSION, auth, events, now_ms};

/// Shared server state.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    token: String,
    port: u16,
    jobs: JobQueue,
    shutdown: Arc<Notify>,
    /// Set once the server stops: ends open event streams.
    stopping: watch::Sender<bool>,
    /// Unix ms of the last authenticated request: how long ago a client was attached.
    last_client_ms: AtomicU64,
    /// Open `GET /v1/events` streams; a client with one open counts as attached.
    open_streams: AtomicUsize,
}

impl AppState {
    /// Must be called inside a Tokio runtime (it starts the job worker).
    /// With a `store`, jobs are persisted there and loaded from it now.
    pub fn new(token: String, port: u16, shutdown: Arc<Notify>, store: Option<JobStore>) -> Self {
        Self {
            inner: Arc::new(Inner {
                token,
                port,
                jobs: JobQueue::start(store),
                shutdown,
                stopping: watch::channel(false).0,
                last_client_ms: AtomicU64::new(now_ms()),
                open_streams: AtomicUsize::new(0),
            }),
        }
    }

    /// The job queue.
    pub fn jobs(&self) -> &JobQueue {
        &self.inner.jobs
    }

    /// Called once the server stops accepting requests: ends event streams so
    /// the graceful shutdown can complete, and abandons running jobs.
    pub(crate) fn begin_stopping(&self) {
        self.inner.stopping.send_replace(true);
        self.inner.jobs.halt();
    }

    /// Jobs queued or running.
    pub fn active_jobs(&self) -> usize {
        self.inner.jobs.active()
    }

    /// Milliseconds since the last authenticated request; 0 while an event
    /// stream is open.
    pub fn client_idle_ms(&self) -> u64 {
        if self.inner.open_streams.load(Ordering::Relaxed) > 0 {
            return 0;
        }
        now_ms().saturating_sub(self.inner.last_client_ms.load(Ordering::Relaxed))
    }
}

/// Marks a client as attached while an event stream is open.
struct StreamGuard(Arc<Inner>);

impl StreamGuard {
    fn new(inner: &Arc<Inner>) -> Self {
        inner.open_streams.fetch_add(1, Ordering::Relaxed);
        Self(inner.clone())
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        self.0.last_client_ms.store(now_ms(), Ordering::Relaxed);
        self.0.open_streams.fetch_sub(1, Ordering::Relaxed);
    }
}

/// All routes, behind the host/origin/token guard.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/tracks", post(tracks))
        .route("/v1/jobs", post(create_job).get(list_jobs))
        .route("/v1/jobs/{id}", get(get_job))
        .route("/v1/jobs/{id}/cancel", post(cancel_job))
        .route("/v1/events", get(job_events))
        .route("/v1/shutdown", post(shutdown))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

struct HttpError(StatusCode, ApiError);

impl HttpError {
    fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self(
            status,
            ApiError {
                code: code.into(),
                message: message.into(),
            },
        )
    }
}

impl From<OpError> for HttpError {
    fn from(error: OpError) -> Self {
        let status =
            StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        Self(status, error.to_api())
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        (self.0, Json(ErrorBody { error: self.1 })).into_response()
    }
}

async fn guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if !auth::host_allowed(host, state.inner.port) {
        return HttpError::new(
            StatusCode::FORBIDDEN,
            error_codes::FORBIDDEN_HOST,
            "requests must address the server as 127.0.0.1, localhost or [::1]",
        )
        .into_response();
    }
    if headers.contains_key(header::ORIGIN) {
        return HttpError::new(
            StatusCode::FORBIDDEN,
            error_codes::FORBIDDEN_ORIGIN,
            "browser requests are not accepted",
        )
        .into_response();
    }
    if request.uri().path() != "/v1/health" {
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        if !auth::bearer_matches(authorization, &state.inner.token) {
            return HttpError::new(
                StatusCode::UNAUTHORIZED,
                error_codes::UNAUTHORIZED,
                "missing or wrong bearer token",
            )
            .into_response();
        }
        state
            .inner
            .last_client_ms
            .store(now_ms(), Ordering::Relaxed);
    }
    next.run(request).await
}

async fn health() -> Json<Health> {
    Json(Health {
        service: SERVICE_NAME.into(),
        version: VERSION.into(),
        api: API_VERSION,
    })
}

async fn tracks(
    body: Result<Json<TracksRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<TracksResponse>, HttpError> {
    let Json(request) = body.map_err(bad_json)?;
    Ok(Json(ops::list_tracks(&request.path).await?))
}

async fn create_job(
    State(state): State<AppState>,
    body: Result<Json<JobSpec>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, Json<JobStatus>), HttpError> {
    let Json(spec) = body.map_err(bad_json)?;
    let plan = match spec {
        JobSpec::TranslateSubtitle(job) => ops::plan_translate(job)?,
    };
    Ok((StatusCode::ACCEPTED, Json(state.inner.jobs.submit(plan))))
}

async fn list_jobs(State(state): State<AppState>) -> Json<JobList> {
    Json(JobList {
        jobs: state.inner.jobs.list(),
    })
}

async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<JobStatus>, HttpError> {
    state
        .inner
        .jobs
        .get(&id)
        .map(Json)
        .ok_or_else(|| no_job(&id))
}

async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<JobStatus>, HttpError> {
    match state.inner.jobs.cancel(&id) {
        Ok(status) => Ok(Json(status)),
        Err(CancelError::NotFound) => Err(no_job(&id)),
        Err(CancelError::NotCancellable(job_state)) => Err(HttpError::new(
            StatusCode::CONFLICT,
            error_codes::NOT_CANCELLABLE,
            match job_state {
                JobState::Running => format!("job {id} has already written its output"),
                other => format!("job {id} already {}", state_name(other)),
            },
        )),
    }
}

fn state_name(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "queued",
        JobState::Running => "running",
        JobState::Succeeded => "succeeded",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    }
}

/// Frames buffered per event stream before the stream task waits for the client.
const EVENT_BUFFER: usize = 32;

async fn job_events(State(state): State<AppState>, uri: Uri) -> Result<Response, HttpError> {
    let filter = events::job_filter(uri.query());
    if let Some(id) = &filter
        && state.inner.jobs.get(id).is_none()
    {
        return Err(no_job(id));
    }
    let (tx, rx) = mpsc::channel::<axum::body::Bytes>(EVENT_BUFFER);
    tokio::spawn(events::run(
        state.inner.jobs.clone(),
        filter,
        state.inner.stopping.subscribe(),
        tx,
        StreamGuard::new(&state.inner),
    ));
    let body = futures_util::stream::unfold(rx, |mut rx| async move {
        let frame = rx.recv().await?;
        Some((Ok::<_, Infallible>(frame), rx))
    });
    Ok((
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Body::from_stream(body),
    )
        .into_response())
}

fn no_job(id: &str) -> HttpError {
    HttpError::new(
        StatusCode::NOT_FOUND,
        error_codes::NOT_FOUND,
        format!("no job {id}"),
    )
}

async fn shutdown(State(state): State<AppState>) -> StatusCode {
    state.inner.shutdown.notify_one();
    StatusCode::ACCEPTED
}

async fn not_found() -> HttpError {
    HttpError::new(
        StatusCode::NOT_FOUND,
        error_codes::NOT_FOUND,
        "no such endpoint",
    )
}

fn bad_json(rejection: axum::extract::rejection::JsonRejection) -> HttpError {
    HttpError::new(
        rejection.status(),
        error_codes::BAD_REQUEST,
        rejection.body_text(),
    )
}
