//! The HTTP API. Every route but `/health` needs the API token.

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use ravenrs_report::Report;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

use crate::config::Config;
use crate::cron::Schedule;
use crate::job::{Delivery, Format, Job, ParameterValue};
use crate::render::{local_date_time, parameter_values, render, resolve_data};
use crate::scheduler::Runner;
use crate::store::{JobSummary, Store, valid_name};

/// The default and largest number of runs a listing returns.
const RUNS_DEFAULT: usize = 50;
const RUNS_MAX: usize = 500;

/// What the handlers share.
#[derive(Clone)]
pub struct AppState {
    store: Store,
    runner: Runner,
    config: Arc<Config>,
    token: Arc<str>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").field("store", &self.store).finish_non_exhaustive()
    }
}

impl AppState {
    /// The state for [`router`].
    pub fn new(store: Store, runner: Runner, config: Arc<Config>, token: String) -> Self {
        Self { store, runner, config, token: token.into() }
    }
}

/// An error answered to the client. Internal details go to the log, not the answer.
enum ApiError {
    BadRequest(String),
    NotFound,
    Conflict(String),
    Internal(String),
}

impl From<io::Error> for ApiError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::InvalidInput {
            Self::BadRequest(e.to_string())
        } else {
            Self::Internal(e.to_string())
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_owned()),
            Self::Conflict(message) => (StatusCode::CONFLICT, message),
            Self::Internal(detail) => {
                tracing::error!(error = %detail, "request failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_owned())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// The API's routes.
pub fn router(state: AppState) -> Router {
    let limit = state.config.max_body_megabytes.saturating_mul(1024 * 1024);
    let protected = Router::new()
        .route("/render", post(render_now))
        .route("/layouts", get(list_layouts))
        .route("/layouts/{name}", get(get_layout).put(put_layout).delete(delete_layout))
        .route("/layouts/{name}/versions", get(layout_versions))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}", get(get_job).put(put_job).delete(delete_job))
        .route("/jobs/{id}/run", post(run_job))
        .route("/jobs/{id}/runs", get(list_runs))
        .route("/jobs/{id}/runs/{run}", get(get_run))
        .route("/jobs/{id}/runs/{run}/documents/{file}", get(get_document))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));
    Router::new()
        .route("/health", get(|| async { Json(json!({ "status": "ok" })) }))
        .merge(protected)
        .layer(DefaultBodyLimit::max(limit))
        .with_state(state)
}

/// Compares without stopping at the first difference, so the time an answer
/// takes says nothing about how much of a guessed token was right.
fn same_secret(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |difference, (x, y)| difference | (x ^ y)) == 0
}

async fn require_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if presented.is_some_and(|token| same_secret(token.as_bytes(), state.token.as_bytes())) {
        return next.run(request).await;
    }
    tracing::warn!(path = %request.uri().path(), "request without a valid token");
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        Json(json!({ "error": "a valid API token is required" })),
    )
        .into_response()
}

fn parse_report(text: &str) -> Result<Report, ApiError> {
    Report::from_json(text).map_err(|e| ApiError::BadRequest(format!("not a valid layout: {e}")))
}

async fn list_layouts(State(state): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(state.store.layouts()?))
}

async fn put_layout(
    State(state): State<AppState>,
    Path(name): Path<String>,
    body: String,
) -> Result<Json<JsonValue>, ApiError> {
    parse_report(&body)?;
    let version = state.store.save_layout(&name, &body)?;
    Ok(Json(json!({ "name": name, "version": version })))
}

#[derive(Deserialize)]
struct VersionQuery {
    version: Option<u32>,
}

async fn get_layout(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<VersionQuery>,
) -> Result<Response, ApiError> {
    let (version, text) = state.store.layout(&name, query.version)?.ok_or(ApiError::NotFound)?;
    Ok((
        [(header::CONTENT_TYPE, "application/json".to_owned()), (header::ETAG, format!("\"{version}\""))],
        text,
    )
        .into_response())
}

async fn layout_versions(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let versions = state.store.layout_versions(&name)?;
    if versions.is_empty() {
        return Err(ApiError::NotFound);
    }
    Ok(Json(versions).into_response())
}

async fn delete_layout(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    for id in state.store.job_ids()? {
        if state.store.job(&id)?.is_some_and(|job| job.layout == name) {
            return Err(ApiError::Conflict(format!("the job '{id}' uses this layout")));
        }
    }
    if state.store.delete_layout(&name)? { Ok(StatusCode::NO_CONTENT) } else { Err(ApiError::NotFound) }
}

/// A request to render now.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenderRequest {
    /// A stored layout's name.
    name: Option<String>,
    /// A version of it; the latest by default.
    version: Option<u32>,
    /// A layout sent with the request, instead of a stored one.
    layout: Option<JsonValue>,
    /// Data sent with the request, instead of the layout's data source.
    data: Option<JsonValue>,
    #[serde(default)]
    parameters: BTreeMap<String, ParameterValue>,
    format: Format,
    /// The time zone of `Today()`.
    timezone: Option<String>,
}

async fn render_now(State(state): State<AppState>, body: Bytes) -> Result<Response, ApiError> {
    let request: RenderRequest =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(format!("bad request: {e}")))?;
    let report = match (&request.name, &request.layout) {
        (Some(name), None) => {
            let (_, text) = state.store.layout(name, request.version)?.ok_or(ApiError::NotFound)?;
            parse_report(&text)?
        }
        (None, Some(layout)) => serde_json::from_value(layout.clone())
            .map_err(|e| ApiError::BadRequest(format!("not a valid layout: {e}")))?,
        _ => return Err(ApiError::BadRequest("send either 'name' or 'layout'".to_owned())),
    };
    let zone = TimeZone::get(request.timezone.as_deref().unwrap_or("UTC"))
        .map_err(|_| ApiError::BadRequest("unknown time zone".to_owned()))?;
    let local = Timestamp::now().to_zoned(zone);
    let now = local_date_time(&local).ok_or_else(|| ApiError::Internal("date out of range".to_owned()))?;
    let date = local.strftime("%Y-%m-%d").to_string();
    let parameters = parameter_values(&request.parameters, now).map_err(ApiError::BadRequest)?;
    let format = request.format;
    let config = state.config.clone();
    let permit =
        state.runner.permits().acquire_owned().await.map_err(|e| ApiError::Internal(e.to_string()))?;
    let rendered = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let data = match request.data {
            Some(data) => data,
            None => resolve_data(&report, &parameters, &config)?,
        };
        render(&report, &data, parameters, now, &[format], &date)
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(ApiError::BadRequest)?;
    let warnings = rendered.warnings.len();
    let file = rendered.files.into_iter().next().ok_or_else(|| ApiError::Internal("no file".to_owned()))?;
    Ok(file_response(&file.name, file.mime, file.bytes, Some(warnings)))
}

/// A file as a download. The name is sent in both of the header's forms, so
/// neither a quote nor an accent in a title can break it.
fn file_response(name: &str, mime: &str, bytes: Vec<u8>, warnings: Option<usize>) -> Response {
    let ascii: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || " .-_".contains(c) { c } else { '_' }).collect();
    let encoded: String = name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let disposition = format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}");
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(mime) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    if let Some(warnings) = warnings {
        headers.insert("x-report-warnings", HeaderValue::from(warnings));
    }
    response
}

async fn list_jobs(State(state): State<AppState>) -> Result<Json<Vec<JobSummary>>, ApiError> {
    let mut jobs = Vec::new();
    for id in state.store.job_ids()? {
        if let Some(job) = state.store.job(&id)? {
            let next_run = state.store.job_state(&id)?.next_run;
            jobs.push(JobSummary { id, job, next_run });
        }
    }
    Ok(Json(jobs))
}

async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<JobSummary>, ApiError> {
    let job = state.store.job(&id)?.ok_or(ApiError::NotFound)?;
    let next_run = state.store.job_state(&id)?.next_run;
    Ok(Json(JobSummary { id, job, next_run }))
}

async fn put_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<JsonValue>, ApiError> {
    if !valid_name(&id) {
        return Err(ApiError::BadRequest(format!("'{id}' is not a valid name")));
    }
    let job: Job =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(format!("bad job: {e}")))?;
    job.validate().map_err(ApiError::BadRequest)?;
    if state.store.layout(&job.layout, None)?.is_none() {
        return Err(ApiError::BadRequest(format!("the layout '{}' does not exist", job.layout)));
    }
    for delivery in &job.deliveries {
        match delivery {
            Delivery::Email { .. } if state.config.smtp.is_none() => {
                return Err(ApiError::BadRequest("the server has no mail server configured".to_owned()));
            }
            Delivery::Folder { .. } if state.config.output_dir.is_none() => {
                return Err(ApiError::BadRequest("the server has no outputDir configured".to_owned()));
            }
            _ => {}
        }
    }
    state.store.save_job(&id, &job)?;
    let schedule = Schedule::parse(&job.schedule).map_err(ApiError::BadRequest)?;
    let zone = TimeZone::get(&job.timezone).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let next_run = schedule.next_after(&Timestamp::now().to_zoned(zone)).map(|z| z.timestamp());
    state.store.save_job_state(&id, &crate::store::JobState { next_run })?;
    Ok(Json(json!({ "id": id, "nextRun": next_run })))
}

async fn delete_job(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    if state.store.delete_job(&id)? { Ok(StatusCode::NO_CONTENT) } else { Err(ApiError::NotFound) }
}

async fn run_job(State(state): State<AppState>, Path(id): Path<String>) -> Result<Response, ApiError> {
    let job = state.store.job(&id)?.ok_or(ApiError::NotFound)?;
    let runner = state.runner.clone();
    let job_id = id.clone();
    tokio::spawn(async move { runner.run_now(&job_id, &job).await });
    Ok((StatusCode::ACCEPTED, Json(json!({ "job": id, "status": "started" }))).into_response())
}

#[derive(Deserialize)]
struct RunsQuery {
    limit: Option<usize>,
}

async fn list_runs(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RunsQuery>,
) -> Result<Response, ApiError> {
    if state.store.job(&id)?.is_none() {
        return Err(ApiError::NotFound);
    }
    let limit = query.limit.unwrap_or(RUNS_DEFAULT).min(RUNS_MAX);
    Ok(Json(state.store.runs(&id, limit)?).into_response())
}

async fn get_run(
    State(state): State<AppState>,
    Path((id, run)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let record = state.store.run(&id, &run)?.ok_or(ApiError::NotFound)?;
    Ok(Json(record).into_response())
}

async fn get_document(
    State(state): State<AppState>,
    Path((id, run, file)): Path<(String, String, String)>,
) -> Result<Response, ApiError> {
    let bytes = state.store.document(&id, &run, &file)?.ok_or(ApiError::NotFound)?;
    let format = match file.rsplit('.').next() {
        Some("pdf") => Format::Pdf,
        Some("xlsx") => Format::Xlsx,
        _ => Format::Csv,
    };
    Ok(file_response(&file, crate::render::mime(format), bytes, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_compare_exactly() {
        assert!(same_secret(b"abc", b"abc"));
        assert!(!same_secret(b"abc", b"abd"));
        assert!(!same_secret(b"abc", b"abcd"));
        assert!(!same_secret(b"", b"a"));
    }

    #[test]
    fn download_names_cannot_break_the_header() {
        let response = file_response("Relatório \"a\"\r\nX: y.pdf", "application/pdf", vec![], None);
        let disposition = response.headers()[header::CONTENT_DISPOSITION].to_str().unwrap_or_default();
        assert!(disposition.contains("filename=\"Relat_rio _a___X_ y.pdf\""), "{disposition}");
        assert!(
            disposition.contains("filename*=UTF-8''Relat%C3%B3rio%20%22a%22%0D%0AX%3A%20y.pdf"),
            "{disposition}"
        );
    }
}
