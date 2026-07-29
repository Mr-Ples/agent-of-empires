use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::github::{
    issue_sync_cache_dir, project_work_items, IssueRef, IssueRepository, IssueSyncMetadata,
    IssueSyncStore, WorkItemListProjection,
};

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

    let attachments: Vec<(IssueRef, String)> = {
        let instances = state.instances.read().await;
        instances
            .iter()
            .filter_map(|inst| {
                inst.issue_ref
                    .as_ref()
                    .map(|issue_ref| (issue_ref.clone(), inst.id.clone()))
            })
            .collect()
    };

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

    let work_items = project_work_items(
        cache.issues,
        attachments
            .iter()
            .map(|(issue_ref, session_id)| (issue_ref, session_id.as_str())),
    );

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
