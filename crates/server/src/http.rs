//! HTTP routes (API v1).

use std::sync::Arc;

use animesubs_api::{
    API_VERSION, ApiError, ErrorBody, Health, JobList, JobSpec, JobStatus, SERVICE_NAME,
    TracksRequest, TracksResponse, error_codes,
};
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::Notify;

use crate::jobs::JobQueue;
use crate::ops::{self, OpError};
use crate::{VERSION, auth};

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
}

impl AppState {
    /// Must be called inside a Tokio runtime (it starts the job worker).
    pub fn new(token: String, port: u16, shutdown: Arc<Notify>) -> Self {
        Self {
            inner: Arc::new(Inner {
                token,
                port,
                jobs: JobQueue::start(),
                shutdown,
            }),
        }
    }
}

/// All routes, behind the host/origin/token guard.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/tracks", post(tracks))
        .route("/v1/jobs", post(create_job).get(list_jobs))
        .route("/v1/jobs/{id}", get(get_job))
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
    state.inner.jobs.get(&id).map(Json).ok_or_else(|| {
        HttpError::new(
            StatusCode::NOT_FOUND,
            error_codes::NOT_FOUND,
            format!("no job {id}"),
        )
    })
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
