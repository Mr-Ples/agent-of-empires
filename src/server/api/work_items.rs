use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::github::{
    issue_sync_cache_dir, project_work_items_with_attention, AttentionInputs, IssueCreateRequest,
    IssueEditRequest, IssueRef, IssueRepository, IssueState, IssueSyncAuthMode, IssueSyncMetadata,
    IssueSyncStore, IssueSyncer, RuntimeLiveness, WorkItemListProjection,
    WorkItemSessionAttachment,
};
use crate::session::{liveness::SessionRuntimeLiveness, Status};

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct WorkItemsQuery {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Serialize)]
pub struct WorkItemsResponse {
    pub repository: IssueRepository,
    pub sync: Option<IssueSyncMetadata>,
    pub work_items: WorkItemListProjection,
}

pub async fn list_work_items(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkItemsQuery>,
) -> impl IntoResponse {
    let repository = match IssueRepository::new(&query.owner, &query.repo) {
        Ok(repository) => repository,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "validation_failed",
                    "message": format!("Invalid repository: {error}"),
                })),
            )
                .into_response();
        }
    };

    let attached_sessions: Vec<AttachedSessionSnapshot> = {
        let instances = state.instances.read().await;
        instances
            .iter()
            .filter_map(|inst| {
                inst.issue_ref
                    .as_ref()
                    .map(|issue_ref| AttachedSessionSnapshot {
                        issue_ref: issue_ref.clone(),
                        session_id: inst.id.clone(),
                        status: inst.status,
                        runtime_liveness: inst.runtime_liveness,
                        runtime_needs_input: inst.runtime_needs_input,
                    })
            })
            .collect()
    };
    let attachments: Vec<WorkItemSessionAttachment> = attached_sessions
        .into_iter()
        .map(|attached| {
            let session_needs_input =
                attached.status == Status::Waiting || attached.runtime_needs_input;
            let structured_needs_input =
                session_has_structured_pending(&state, &attached.session_id);
            WorkItemSessionAttachment {
                issue_ref: attached.issue_ref,
                session_id: attached.session_id,
                attention: AttentionInputs {
                    runtime_liveness: attached
                        .runtime_liveness
                        .map(runtime_liveness_from_session)
                        .unwrap_or_else(|| runtime_liveness_from_status(attached.status)),
                    lifecycle_needs_input: session_needs_input,
                    structured_needs_input,
                },
            }
        })
        .collect();

    let load_repository = repository.clone();
    let cache = match tokio::task::spawn_blocking(move || {
        let app_dir = crate::session::get_app_dir()?;
        let store = IssueSyncStore::new(issue_sync_cache_dir(app_dir));
        store.load(&load_repository).map_err(anyhow::Error::from)
    })
    .await
    {
        Ok(Ok(cache)) => cache,
        Ok(Err(error)) => {
            tracing::warn!(target: "http.api.work_items", %error, "failed to load work item cache");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "work_items_unavailable",
                    "message": "Failed to load cached work items",
                })),
            )
                .into_response();
        }
        Err(error) => {
            tracing::warn!(target: "http.api.work_items", %error, "work item cache task failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "work_items_unavailable",
                    "message": "Failed to load cached work items",
                })),
            )
                .into_response();
        }
    };

    let Some(cache) = cache else {
        return (
            StatusCode::OK,
            Json(WorkItemsResponse {
                repository,
                sync: None,
                work_items: WorkItemListProjection {
                    open: Vec::new(),
                    closed: Vec::new(),
                },
            }),
        )
            .into_response();
    };

    let work_items = project_work_items_with_attention(cache.issues, attachments);

    (
        StatusCode::OK,
        Json(WorkItemsResponse {
            repository,
            sync: Some(cache.sync),
            work_items,
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueBody {
    pub owner: String,
    pub repo: String,
    pub title: String,
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EditIssueBody {
    pub title: Option<String>,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct IssueStateBody {
    pub state: IssueState,
}

fn github_issue_error(error: crate::github::IssueMutationError) -> axum::response::Response {
    let message = error.to_string();
    let status = if message.contains("authentication") {
        StatusCode::UNAUTHORIZED
    } else if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::BAD_GATEWAY
    };
    (
        status,
        Json(serde_json::json!({ "error": "github_issue_mutation_failed", "message": message })),
    )
        .into_response()
}

fn github_client() -> Result<crate::github::GitHubClient, String> {
    let token = ["GITHUB_TOKEN", "GH_TOKEN"]
        .into_iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| "GitHub authentication is required".to_string())?;
    crate::github::GitHubClient::authenticated(
        crate::github::GitHubClientConfig {
            api_base: crate::github::DEFAULT_GITHUB_API_BASE.to_string(),
            user_agent: crate::github::DEFAULT_USER_AGENT.to_string(),
            timeout: std::time::Duration::from_secs(20),
        },
        &token,
    )
    .map_err(|error| error.to_string())
}

async fn issue_syncer() -> Result<IssueSyncer<crate::github::GitHubClient>, String> {
    let app_dir = crate::session::get_app_dir().map_err(|error| error.to_string())?;
    Ok(IssueSyncer::new(
        github_client()?,
        IssueSyncStore::new(issue_sync_cache_dir(app_dir)),
    ))
}

pub async fn create_issue(
    State(state): State<Arc<AppState>>,
    body: Result<Json<CreateIssueBody>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    if state.read_only {
        return super::read_only_response();
    }
    let Json(body) = match body {
        Ok(body) => body,
        Err(rejection) => return rejection.into_response(),
    };
    let repository = match IssueRepository::new(&body.owner, &body.repo) {
        Ok(repository) => repository,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "message": error.to_string() })),
            )
                .into_response()
        }
    };
    let service = match issue_syncer().await {
        Ok(service) => service,
        Err(message) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "message": message })),
            )
                .into_response()
        }
    };
    let request = IssueCreateRequest {
        title: body.title,
        body: body.body,
        labels: Vec::new(),
        apply_default_triage_label: true,
    };
    match service
        .create_issue(&repository, &request, IssueSyncAuthMode::Interactive)
        .await
    {
        Ok(snapshot) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "issue": snapshot.issue })),
        )
            .into_response(),
        Err(error) => github_issue_error(error),
    }
}

pub async fn edit_issue(
    State(state): State<Arc<AppState>>,
    Path((owner, repo, number)): Path<(String, String, u64)>,
    body: Result<Json<EditIssueBody>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    if state.read_only {
        return super::read_only_response();
    }
    let Json(body) = match body {
        Ok(body) => body,
        Err(rejection) => return rejection.into_response(),
    };
    let issue_ref = match IssueRef::new(&owner, &repo, number) {
        Ok(issue_ref) => issue_ref,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "message": error.to_string() })),
            )
                .into_response()
        }
    };
    let service = match issue_syncer().await {
        Ok(service) => service,
        Err(message) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "message": message })),
            )
                .into_response()
        }
    };
    match service
        .edit_issue(
            &issue_ref,
            &IssueEditRequest {
                title: body.title,
                body: body.body,
                labels: body.labels,
            },
            IssueSyncAuthMode::Interactive,
        )
        .await
    {
        Ok(snapshot) => (
            StatusCode::OK,
            Json(serde_json::json!({ "issue": snapshot.issue })),
        )
            .into_response(),
        Err(error) => github_issue_error(error),
    }
}

pub async fn set_issue_state(
    State(state): State<Arc<AppState>>,
    Path((owner, repo, number)): Path<(String, String, u64)>,
    body: Result<Json<IssueStateBody>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    if state.read_only {
        return super::read_only_response();
    }
    let Json(body) = match body {
        Ok(body) => body,
        Err(rejection) => return rejection.into_response(),
    };
    let issue_ref = match IssueRef::new(&owner, &repo, number) {
        Ok(issue_ref) => issue_ref,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "message": error.to_string() })),
            )
                .into_response()
        }
    };
    let service = match issue_syncer().await {
        Ok(service) => service,
        Err(message) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "message": message })),
            )
                .into_response()
        }
    };
    match service
        .set_issue_state_at(
            &issue_ref,
            body.state,
            IssueSyncAuthMode::Interactive,
            chrono::Utc::now(),
        )
        .await
    {
        Ok(snapshot) => (
            StatusCode::OK,
            Json(serde_json::json!({ "issue": snapshot.issue })),
        )
            .into_response(),
        Err(error) => github_issue_error(error),
    }
}

struct AttachedSessionSnapshot {
    issue_ref: IssueRef,
    session_id: String,
    status: Status,
    runtime_liveness: Option<SessionRuntimeLiveness>,
    runtime_needs_input: bool,
}

fn runtime_liveness_from_session(liveness: SessionRuntimeLiveness) -> RuntimeLiveness {
    match liveness {
        SessionRuntimeLiveness::Active => RuntimeLiveness::Active,
        SessionRuntimeLiveness::Idle => RuntimeLiveness::Idle,
        SessionRuntimeLiveness::Stopped => RuntimeLiveness::Stopped,
        SessionRuntimeLiveness::Error => RuntimeLiveness::Error,
    }
}

fn runtime_liveness_from_status(status: Status) -> RuntimeLiveness {
    match status {
        Status::Running | Status::Starting | Status::Creating => RuntimeLiveness::Active,
        Status::Waiting | Status::Idle => RuntimeLiveness::Idle,
        Status::Stopped | Status::Deleting => RuntimeLiveness::Stopped,
        Status::Unknown | Status::Error => RuntimeLiveness::Error,
    }
}

#[cfg(feature = "serve")]
fn session_has_structured_pending(state: &AppState, session_id: &str) -> bool {
    !state
        .acp_event_store
        .unresolved_approval_nonces(session_id)
        .is_empty()
        || !state
            .acp_event_store
            .unresolved_elicitation_nonces(session_id)
            .is_empty()
}

#[cfg(not(feature = "serve"))]
fn session_has_structured_pending(_state: &AppState, _session_id: &str) -> bool {
    false
}
