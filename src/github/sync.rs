//! GitHub Issue sync service and normalized cache storage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::github::{
    GitHubError, GitHubIssuePayload, IssueCreateRequest, IssueEditRequest,
    IssueMutationValidationError, IssueNormalizeError, IssueRecord, IssueRef, IssueState,
    IssueSyncFailureKind, IssueSyncMetadata, IssueSyncStatus,
};

pub const ISSUE_SYNC_CACHE_SUBDIR: &str = "github/issues";

pub type IssueClientFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<GitHubIssuePayload>, IssueSyncFailure>> + Send + 'a>>;
pub type IssuePayloadFuture<'a> =
    Pin<Box<dyn Future<Output = Result<GitHubIssuePayload, IssueSyncFailure>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRepository {
    pub owner: String,
    pub repo: String,
}

impl IssueRepository {
    pub fn new(owner: impl AsRef<str>, repo: impl AsRef<str>) -> Result<Self, IssueNormalizeError> {
        let issue_ref = crate::github::IssueRef::new(owner, repo, 1)?;
        Ok(Self {
            owner: issue_ref.owner().to_string(),
            repo: issue_ref.repo().to_string(),
        })
    }

    fn cache_slug(&self) -> String {
        format!("{}__{}.json", self.owner, self.repo)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSyncAuthMode {
    Interactive,
    NonInteractive,
}

impl IssueSyncAuthMode {
    fn is_interactive(self) -> bool {
        self == IssueSyncAuthMode::Interactive
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSyncFailure {
    pub kind: IssueSyncFailureKind,
    pub message: String,
}

impl IssueSyncFailure {
    pub fn new(kind: IssueSyncFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn auth_required(mode: IssueSyncAuthMode, message: impl Into<String>) -> Self {
        Self::new(
            IssueSyncFailureKind::AuthRequired {
                interactive: mode.is_interactive(),
            },
            message,
        )
    }

    pub fn from_github_error(value: GitHubError, auth_mode: IssueSyncAuthMode) -> Self {
        match value {
            GitHubError::Unauthorized => {
                Self::auth_required(auth_mode, "GitHub authentication is required")
            }
            GitHubError::InsufficientScope { scopes } => Self::new(
                IssueSyncFailureKind::Forbidden,
                format!("GitHub token is missing required scope: {scopes}"),
            ),
            GitHubError::RateLimited => Self::new(
                IssueSyncFailureKind::RateLimited,
                "GitHub API rate limit exceeded",
            ),
            GitHubError::NotFound { resource } => {
                Self::new(IssueSyncFailureKind::NotFound, resource)
            }
            GitHubError::Network { source } => {
                Self::new(IssueSyncFailureKind::Network, source.to_string())
            }
            GitHubError::Api { status, message } if status == reqwest::StatusCode::FORBIDDEN => {
                Self::new(IssueSyncFailureKind::Forbidden, message)
            }
            GitHubError::Api { message, .. } => {
                Self::new(IssueSyncFailureKind::ApiFailure, message)
            }
            GitHubError::Decode(source) | GitHubError::Http(source) => {
                Self::new(IssueSyncFailureKind::ApiFailure, source.to_string())
            }
            GitHubError::InvalidHeader(message) => {
                Self::new(IssueSyncFailureKind::ApiFailure, message)
            }
        }
    }

    fn status(&self) -> IssueSyncStatus {
        match self.kind {
            IssueSyncFailureKind::AuthRequired { .. } => IssueSyncStatus::AuthRequired,
            IssueSyncFailureKind::Forbidden => IssueSyncStatus::Forbidden,
            IssueSyncFailureKind::NotFound => IssueSyncStatus::NotFound,
            IssueSyncFailureKind::Network => IssueSyncStatus::Network,
            IssueSyncFailureKind::RateLimited => IssueSyncStatus::RateLimited,
            IssueSyncFailureKind::ApiFailure => IssueSyncStatus::ApiFailure,
        }
    }

    fn metadata_without_cache(&self) -> IssueSyncMetadata {
        IssueSyncMetadata {
            status: self.status(),
            synced_at: None,
            message: Some(self.message.clone()),
            failure: Some(self.kind.clone()),
        }
    }

    fn stale_metadata(&self) -> IssueSyncMetadata {
        IssueSyncMetadata::stale(self.kind.clone(), self.message.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSyncCache {
    pub repository: IssueRepository,
    pub issues: Vec<IssueRecord>,
    pub sync: IssueSyncMetadata,
}

impl IssueSyncCache {
    fn empty(repository: IssueRepository) -> Self {
        Self {
            repository,
            issues: Vec::new(),
            sync: IssueSyncMetadata {
                status: IssueSyncStatus::Fresh,
                synced_at: None,
                message: None,
                failure: None,
            },
        }
    }

    fn with_metadata(mut self, metadata: IssueSyncMetadata) -> Self {
        for issue in &mut self.issues {
            issue.sync = metadata.clone();
        }
        self.sync = metadata;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueSyncSnapshot {
    pub cache: IssueSyncCache,
    pub used_stale_cache: bool,
}

impl IssueSyncSnapshot {
    pub fn issues(&self) -> &[IssueRecord] {
        &self.cache.issues
    }

    pub fn sync(&self) -> &IssueSyncMetadata {
        &self.cache.sync
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IssueSyncError {
    #[error("failed to read issue cache: {0}")]
    Read(#[source] std::io::Error),
    #[error("failed to create issue cache directory: {0}")]
    CreateDir(#[source] std::io::Error),
    #[error("failed to parse issue cache: {0}")]
    Parse(#[source] serde_json::Error),
    #[error("failed to write issue cache: {0}")]
    Write(#[source] anyhow::Error),
    #[error("failed to normalize GitHub issue payload: {0}")]
    Normalize(#[from] IssueNormalizeError),
}

#[derive(Debug, thiserror::Error)]
pub enum IssueMutationError {
    #[error(transparent)]
    Validation(#[from] IssueMutationValidationError),
    #[error(transparent)]
    Normalize(#[from] IssueNormalizeError),
    #[error(transparent)]
    Cache(#[from] IssueSyncError),
    #[error("GitHub issue mutation failed: {0}", .failure.message)]
    GitHub { failure: IssueSyncFailure },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueMutationSnapshot {
    pub issue: IssueRecord,
    pub cache: IssueSyncCache,
}

pub trait GitHubIssueClient: Send + Sync {
    fn list_repo_issues<'a>(
        &'a self,
        repository: &'a IssueRepository,
        auth_mode: IssueSyncAuthMode,
    ) -> IssueClientFuture<'a>;

    fn repo_issue<'a>(
        &'a self,
        issue_ref: &'a IssueRef,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a>;

    fn create_repo_issue<'a>(
        &'a self,
        repository: &'a IssueRepository,
        request: &'a crate::github::ValidatedIssueCreateRequest,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a>;

    fn edit_repo_issue<'a>(
        &'a self,
        issue_ref: &'a IssueRef,
        request: &'a crate::github::ValidatedIssueEditRequest,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a>;

    fn set_repo_issue_state<'a>(
        &'a self,
        issue_ref: &'a IssueRef,
        state: IssueState,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a>;
}

#[derive(Debug, Clone)]
pub struct IssueSyncStore {
    root: PathBuf,
}

pub fn issue_sync_cache_dir(app_dir: impl Into<PathBuf>) -> PathBuf {
    app_dir.into().join(ISSUE_SYNC_CACHE_SUBDIR)
}

impl IssueSyncStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path_for(&self, repository: &IssueRepository) -> PathBuf {
        self.root.join(repository.cache_slug())
    }

    pub fn load(
        &self,
        repository: &IssueRepository,
    ) -> Result<Option<IssueSyncCache>, IssueSyncError> {
        let path = self.path_for(repository);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(IssueSyncError::Read)?;
        serde_json::from_str(&content)
            .map(Some)
            .map_err(IssueSyncError::Parse)
    }

    pub fn save(&self, cache: &IssueSyncCache) -> Result<(), IssueSyncError> {
        let path = self.path_for(&cache.repository);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(IssueSyncError::CreateDir)?;
        }
        let content = serde_json::to_string_pretty(cache).map_err(IssueSyncError::Parse)?;
        crate::session::atomic_write(&path, content.as_bytes()).map_err(IssueSyncError::Write)
    }
}

pub struct IssueSyncer<C> {
    client: C,
    store: IssueSyncStore,
}

impl<C> IssueSyncer<C>
where
    C: GitHubIssueClient,
{
    pub fn new(client: C, store: IssueSyncStore) -> Self {
        Self { client, store }
    }

    pub async fn sync(
        &self,
        repository: &IssueRepository,
        auth_mode: IssueSyncAuthMode,
    ) -> Result<IssueSyncSnapshot, IssueSyncError> {
        self.sync_at(repository, auth_mode, Utc::now()).await
    }

    pub async fn sync_at(
        &self,
        repository: &IssueRepository,
        auth_mode: IssueSyncAuthMode,
        synced_at: DateTime<Utc>,
    ) -> Result<IssueSyncSnapshot, IssueSyncError> {
        let cached = self.store.load(repository)?;
        match self.client.list_repo_issues(repository, auth_mode).await {
            Ok(payloads) => {
                let sync = IssueSyncMetadata::fresh(synced_at);
                let mut issues = Vec::with_capacity(payloads.len());
                for payload in payloads {
                    issues.push(payload.normalize(
                        &repository.owner,
                        &repository.repo,
                        sync.clone(),
                    )?);
                }
                let cache = IssueSyncCache {
                    repository: repository.clone(),
                    issues,
                    sync,
                };
                self.store.save(&cache)?;
                Ok(IssueSyncSnapshot {
                    cache,
                    used_stale_cache: false,
                })
            }
            Err(failure) => {
                let used_stale_cache = cached.is_some();
                let cache = cached
                    .unwrap_or_else(|| IssueSyncCache::empty(repository.clone()))
                    .with_metadata(if used_stale_cache {
                        failure.stale_metadata()
                    } else {
                        failure.metadata_without_cache()
                    });
                self.store.save(&cache)?;
                Ok(IssueSyncSnapshot {
                    cache,
                    used_stale_cache,
                })
            }
        }
    }

    pub async fn create_issue(
        &self,
        repository: &IssueRepository,
        request: &IssueCreateRequest,
        auth_mode: IssueSyncAuthMode,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        self.create_issue_at(repository, request, auth_mode, Utc::now())
            .await
    }

    pub async fn create_issue_at(
        &self,
        repository: &IssueRepository,
        request: &IssueCreateRequest,
        auth_mode: IssueSyncAuthMode,
        synced_at: DateTime<Utc>,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        let request = request.validated()?;
        let payload = match self
            .client
            .create_repo_issue(repository, &request, auth_mode)
            .await
        {
            Ok(payload) => payload,
            Err(failure) => {
                let failure = self.persist_failure(repository, failure)?;
                return Err(IssueMutationError::GitHub { failure });
            }
        };
        self.persist_mutated_payload(repository, payload, synced_at)
    }

    pub async fn refresh_issue(
        &self,
        issue_ref: &IssueRef,
        auth_mode: IssueSyncAuthMode,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        self.refresh_issue_at(issue_ref, auth_mode, Utc::now())
            .await
    }

    pub async fn refresh_issue_at(
        &self,
        issue_ref: &IssueRef,
        auth_mode: IssueSyncAuthMode,
        synced_at: DateTime<Utc>,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        let repository = IssueRepository::new(issue_ref.owner(), issue_ref.repo())?;
        let payload = match self.client.repo_issue(issue_ref, auth_mode).await {
            Ok(payload) => payload,
            Err(failure) => {
                let failure = self.persist_failure(&repository, failure)?;
                return Err(IssueMutationError::GitHub { failure });
            }
        };
        self.persist_mutated_payload(&repository, payload, synced_at)
    }

    pub async fn edit_issue(
        &self,
        issue_ref: &IssueRef,
        request: &IssueEditRequest,
        auth_mode: IssueSyncAuthMode,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        self.edit_issue_at(issue_ref, request, auth_mode, Utc::now())
            .await
    }

    pub async fn edit_issue_at(
        &self,
        issue_ref: &IssueRef,
        request: &IssueEditRequest,
        auth_mode: IssueSyncAuthMode,
        synced_at: DateTime<Utc>,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        let request = request.validated()?;
        let repository = IssueRepository::new(issue_ref.owner(), issue_ref.repo())?;
        let payload = match self
            .client
            .edit_repo_issue(issue_ref, &request, auth_mode)
            .await
        {
            Ok(payload) => payload,
            Err(failure) => {
                let failure = self.persist_failure(&repository, failure)?;
                return Err(IssueMutationError::GitHub { failure });
            }
        };
        self.persist_mutated_payload(&repository, payload, synced_at)
    }

    pub async fn close_issue(
        &self,
        issue_ref: &IssueRef,
        auth_mode: IssueSyncAuthMode,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        self.set_issue_state_at(issue_ref, IssueState::Closed, auth_mode, Utc::now())
            .await
    }

    pub async fn reopen_issue(
        &self,
        issue_ref: &IssueRef,
        auth_mode: IssueSyncAuthMode,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        self.set_issue_state_at(issue_ref, IssueState::Open, auth_mode, Utc::now())
            .await
    }

    pub async fn set_issue_state_at(
        &self,
        issue_ref: &IssueRef,
        state: IssueState,
        auth_mode: IssueSyncAuthMode,
        synced_at: DateTime<Utc>,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        let repository = IssueRepository::new(issue_ref.owner(), issue_ref.repo())?;
        let payload = match self
            .client
            .set_repo_issue_state(issue_ref, state, auth_mode)
            .await
        {
            Ok(payload) => payload,
            Err(failure) => {
                let failure = self.persist_failure(&repository, failure)?;
                return Err(IssueMutationError::GitHub { failure });
            }
        };
        self.persist_mutated_payload(&repository, payload, synced_at)
    }

    fn persist_mutated_payload(
        &self,
        repository: &IssueRepository,
        payload: GitHubIssuePayload,
        synced_at: DateTime<Utc>,
    ) -> Result<IssueMutationSnapshot, IssueMutationError> {
        let sync = IssueSyncMetadata::fresh(synced_at);
        let issue = payload.normalize(&repository.owner, &repository.repo, sync.clone())?;
        let mut cache = self
            .store
            .load(repository)?
            .unwrap_or_else(|| IssueSyncCache::empty(repository.clone()));
        let had_existing_cache = cache.sync.synced_at.is_some()
            || !cache.issues.is_empty()
            || cache.sync.status != IssueSyncStatus::Fresh;
        if let Some(existing) = cache
            .issues
            .iter_mut()
            .find(|existing| existing.issue_ref == issue.issue_ref)
        {
            *existing = issue.clone();
        } else {
            cache.issues.push(issue.clone());
        }
        if !had_existing_cache {
            cache.sync = sync;
        }
        self.store.save(&cache)?;
        Ok(IssueMutationSnapshot { issue, cache })
    }

    fn persist_failure(
        &self,
        repository: &IssueRepository,
        failure: IssueSyncFailure,
    ) -> Result<IssueSyncFailure, IssueMutationError> {
        let cached = self.store.load(repository)?;
        let used_stale_cache = cached.is_some();
        let cache = cached
            .unwrap_or_else(|| IssueSyncCache::empty(repository.clone()))
            .with_metadata(if used_stale_cache {
                failure.stale_metadata()
            } else {
                failure.metadata_without_cache()
            });
        self.store.save(&cache)?;
        Ok(failure)
    }
}

impl From<GitHubError> for IssueSyncFailure {
    fn from(value: GitHubError) -> Self {
        Self::from_github_error(value, IssueSyncAuthMode::NonInteractive)
    }
}

impl GitHubIssueClient for crate::github::GitHubClient {
    fn list_repo_issues<'a>(
        &'a self,
        repository: &'a IssueRepository,
        auth_mode: IssueSyncAuthMode,
    ) -> IssueClientFuture<'a> {
        Box::pin(async move {
            self.list_issues(&repository.owner, &repository.repo)
                .await
                .map_err(|error| IssueSyncFailure::from_github_error(error, auth_mode))
        })
    }

    fn repo_issue<'a>(
        &'a self,
        issue_ref: &'a IssueRef,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a> {
        Box::pin(async move {
            self.issue(issue_ref.owner(), issue_ref.repo(), issue_ref.number())
                .await
                .map_err(|error| IssueSyncFailure::from_github_error(error, auth_mode))
        })
    }

    fn create_repo_issue<'a>(
        &'a self,
        repository: &'a IssueRepository,
        request: &'a crate::github::ValidatedIssueCreateRequest,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a> {
        Box::pin(async move {
            self.create_issue(&repository.owner, &repository.repo, request)
                .await
                .map_err(|error| IssueSyncFailure::from_github_error(error, auth_mode))
        })
    }

    fn edit_repo_issue<'a>(
        &'a self,
        issue_ref: &'a IssueRef,
        request: &'a crate::github::ValidatedIssueEditRequest,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a> {
        Box::pin(async move {
            self.edit_issue(
                issue_ref.owner(),
                issue_ref.repo(),
                issue_ref.number(),
                request,
            )
            .await
            .map_err(|error| IssueSyncFailure::from_github_error(error, auth_mode))
        })
    }

    fn set_repo_issue_state<'a>(
        &'a self,
        issue_ref: &'a IssueRef,
        state: IssueState,
        auth_mode: IssueSyncAuthMode,
    ) -> IssuePayloadFuture<'a> {
        Box::pin(async move {
            self.set_issue_state(
                issue_ref.owner(),
                issue_ref.repo(),
                issue_ref.number(),
                state,
            )
            .await
            .map_err(|error| IssueSyncFailure::from_github_error(error, auth_mode))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::issues::GitHubIssueLabelPayload;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn ts(raw: &str) -> DateTime<Utc> {
        raw.parse().unwrap()
    }

    fn payload(number: u64, title: &str) -> GitHubIssuePayload {
        GitHubIssuePayload {
            id: 1000 + number,
            node_id: format!("I_{number}"),
            number,
            title: title.to_string(),
            body: Some(format!("Body for {title}")),
            state: "open".to_string(),
            labels: Vec::new(),
            assignees: Vec::new(),
            html_url: format!("https://github.com/Mr-Ples/agent-of-empires/issues/{number}"),
            created_at: ts("2026-07-01T12:00:00Z"),
            updated_at: ts("2026-07-02T12:00:00Z"),
            closed_at: None,
            pull_request: None,
        }
    }

    struct FakeGitHubIssueClient {
        responses: Mutex<VecDeque<Result<Vec<GitHubIssuePayload>, IssueSyncFailure>>>,
        payload_responses: Mutex<VecDeque<Result<GitHubIssuePayload, IssueSyncFailure>>>,
        auth_modes: Mutex<Vec<IssueSyncAuthMode>>,
        mutation_calls: Mutex<Vec<String>>,
    }

    impl FakeGitHubIssueClient {
        fn new(responses: Vec<Result<Vec<GitHubIssuePayload>, IssueSyncFailure>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                payload_responses: Mutex::new(VecDeque::new()),
                auth_modes: Mutex::new(Vec::new()),
                mutation_calls: Mutex::new(Vec::new()),
            }
        }

        fn auth_modes(&self) -> Vec<IssueSyncAuthMode> {
            self.auth_modes.lock().unwrap().clone()
        }

        fn with_payload_responses(
            payload_responses: Vec<Result<GitHubIssuePayload, IssueSyncFailure>>,
        ) -> Self {
            Self {
                responses: Mutex::new(VecDeque::new()),
                payload_responses: Mutex::new(payload_responses.into()),
                auth_modes: Mutex::new(Vec::new()),
                mutation_calls: Mutex::new(Vec::new()),
            }
        }

        fn mutation_calls(&self) -> Vec<String> {
            self.mutation_calls.lock().unwrap().clone()
        }
    }

    impl GitHubIssueClient for FakeGitHubIssueClient {
        fn list_repo_issues<'a>(
            &'a self,
            _repository: &'a IssueRepository,
            auth_mode: IssueSyncAuthMode,
        ) -> IssueClientFuture<'a> {
            Box::pin(async move {
                self.auth_modes.lock().unwrap().push(auth_mode);
                self.responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("fake response queued")
            })
        }

        fn repo_issue<'a>(
            &'a self,
            issue_ref: &'a IssueRef,
            auth_mode: IssueSyncAuthMode,
        ) -> IssuePayloadFuture<'a> {
            Box::pin(async move {
                self.auth_modes.lock().unwrap().push(auth_mode);
                self.mutation_calls
                    .lock()
                    .unwrap()
                    .push(format!("refresh:{issue_ref}"));
                self.payload_responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("fake payload response queued")
            })
        }

        fn create_repo_issue<'a>(
            &'a self,
            repository: &'a IssueRepository,
            request: &'a crate::github::ValidatedIssueCreateRequest,
            auth_mode: IssueSyncAuthMode,
        ) -> IssuePayloadFuture<'a> {
            Box::pin(async move {
                self.auth_modes.lock().unwrap().push(auth_mode);
                self.mutation_calls.lock().unwrap().push(format!(
                    "create:{}:{}:{:?}",
                    repository.owner, request.title, request.labels
                ));
                self.payload_responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("fake payload response queued")
            })
        }

        fn edit_repo_issue<'a>(
            &'a self,
            issue_ref: &'a IssueRef,
            request: &'a crate::github::ValidatedIssueEditRequest,
            auth_mode: IssueSyncAuthMode,
        ) -> IssuePayloadFuture<'a> {
            Box::pin(async move {
                self.auth_modes.lock().unwrap().push(auth_mode);
                self.mutation_calls.lock().unwrap().push(format!(
                    "edit:{}:{:?}:{:?}",
                    issue_ref, request.title, request.labels
                ));
                self.payload_responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("fake payload response queued")
            })
        }

        fn set_repo_issue_state<'a>(
            &'a self,
            issue_ref: &'a IssueRef,
            state: IssueState,
            auth_mode: IssueSyncAuthMode,
        ) -> IssuePayloadFuture<'a> {
            Box::pin(async move {
                self.auth_modes.lock().unwrap().push(auth_mode);
                self.mutation_calls
                    .lock()
                    .unwrap()
                    .push(format!("state:{issue_ref}:{state:?}"));
                self.payload_responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("fake payload response queued")
            })
        }
    }

    fn syncer(
        responses: Vec<Result<Vec<GitHubIssuePayload>, IssueSyncFailure>>,
    ) -> (IssueSyncer<FakeGitHubIssueClient>, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let store = IssueSyncStore::new(temp.path().join("issues"));
        (
            IssueSyncer::new(FakeGitHubIssueClient::new(responses), store),
            temp,
        )
    }

    fn mutation_syncer(
        payload_responses: Vec<Result<GitHubIssuePayload, IssueSyncFailure>>,
    ) -> (IssueSyncer<FakeGitHubIssueClient>, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let store = IssueSyncStore::new(temp.path().join("issues"));
        (
            IssueSyncer::new(
                FakeGitHubIssueClient::with_payload_responses(payload_responses),
                store,
            ),
            temp,
        )
    }

    fn repository() -> IssueRepository {
        IssueRepository::new("Mr-Ples", "agent-of-empires").unwrap()
    }

    #[tokio::test]
    async fn successful_sync_persists_normalized_issue_cache() {
        let repo = repository();
        let (syncer, _temp) = syncer(vec![Ok(vec![payload(14, "Add sync")])]);

        let snapshot = syncer
            .sync_at(
                &repo,
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T09:00:00Z"),
            )
            .await
            .unwrap();

        assert!(!snapshot.used_stale_cache);
        assert_eq!(snapshot.sync().status, IssueSyncStatus::Fresh);
        assert_eq!(
            snapshot.issues()[0].issue_ref.to_string(),
            "mr-ples/agent-of-empires#14"
        );
        assert_eq!(
            snapshot.issues()[0].sync.synced_at,
            Some(ts("2026-07-29T09:00:00Z"))
        );
        let persisted = syncer.store.load(&repo).unwrap().unwrap();
        assert_eq!(persisted.issues[0].title, "Add sync");
    }

    #[tokio::test]
    async fn failed_sync_preserves_last_successful_cache_as_stale() {
        let repo = repository();
        let (syncer, _temp) = syncer(vec![
            Ok(vec![payload(14, "Cached issue")]),
            Err(IssueSyncFailure::new(
                IssueSyncFailureKind::Network,
                "offline",
            )),
        ]);

        syncer
            .sync_at(
                &repo,
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T09:00:00Z"),
            )
            .await
            .unwrap();
        let snapshot = syncer
            .sync_at(
                &repo,
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T09:01:00Z"),
            )
            .await
            .unwrap();

        assert!(snapshot.used_stale_cache);
        assert_eq!(snapshot.issues()[0].title, "Cached issue");
        assert_eq!(snapshot.sync().status, IssueSyncStatus::Stale);
        assert_eq!(snapshot.sync().failure, Some(IssueSyncFailureKind::Network));
        assert_eq!(snapshot.issues()[0].sync.status, IssueSyncStatus::Stale);
    }

    #[tokio::test]
    async fn failed_sync_without_cache_records_typed_failure_state() {
        let repo = repository();
        let cases = [
            (
                IssueSyncFailure::auth_required(IssueSyncAuthMode::NonInteractive, "login needed"),
                IssueSyncStatus::AuthRequired,
                IssueSyncFailureKind::AuthRequired { interactive: false },
            ),
            (
                IssueSyncFailure::new(IssueSyncFailureKind::Forbidden, "forbidden"),
                IssueSyncStatus::Forbidden,
                IssueSyncFailureKind::Forbidden,
            ),
            (
                IssueSyncFailure::new(IssueSyncFailureKind::NotFound, "missing"),
                IssueSyncStatus::NotFound,
                IssueSyncFailureKind::NotFound,
            ),
            (
                IssueSyncFailure::new(IssueSyncFailureKind::Network, "offline"),
                IssueSyncStatus::Network,
                IssueSyncFailureKind::Network,
            ),
            (
                IssueSyncFailure::new(IssueSyncFailureKind::RateLimited, "slow down"),
                IssueSyncStatus::RateLimited,
                IssueSyncFailureKind::RateLimited,
            ),
            (
                IssueSyncFailure::new(IssueSyncFailureKind::ApiFailure, "boom"),
                IssueSyncStatus::ApiFailure,
                IssueSyncFailureKind::ApiFailure,
            ),
        ];

        for (failure, status, kind) in cases {
            let (syncer, _temp) = syncer(vec![Err(failure)]);
            let snapshot = syncer
                .sync_at(
                    &repo,
                    IssueSyncAuthMode::NonInteractive,
                    ts("2026-07-29T09:00:00Z"),
                )
                .await
                .unwrap();

            assert!(!snapshot.used_stale_cache);
            assert!(snapshot.issues().is_empty());
            assert_eq!(snapshot.sync().status, status);
            assert_eq!(snapshot.sync().failure, Some(kind));
        }
    }

    #[tokio::test]
    async fn auth_required_records_interactive_mode_at_sync_boundary() {
        let repo = repository();
        let (syncer, _temp) = syncer(vec![Err(IssueSyncFailure::auth_required(
            IssueSyncAuthMode::Interactive,
            "start auth",
        ))]);

        let snapshot = syncer
            .sync_at(
                &repo,
                IssueSyncAuthMode::Interactive,
                ts("2026-07-29T09:00:00Z"),
            )
            .await
            .unwrap();

        assert_eq!(
            snapshot.sync().failure,
            Some(IssueSyncFailureKind::AuthRequired { interactive: true })
        );
        assert_eq!(
            syncer.client.auth_modes(),
            vec![IssueSyncAuthMode::Interactive]
        );
    }

    #[tokio::test]
    async fn create_issue_validates_request_applies_default_label_and_updates_cache() {
        let repo = repository();
        let created = GitHubIssuePayload {
            number: 21,
            title: "Created issue".to_string(),
            labels: vec![GitHubIssueLabelPayload {
                name: "needs-triage".to_string(),
                color: Some("ffffff".to_string()),
                description: None,
            }],
            ..payload(21, "Created issue")
        };
        let (syncer, _temp) = mutation_syncer(vec![Ok(created)]);
        let request = IssueCreateRequest {
            title: " Created issue ".to_string(),
            body: None,
            labels: Vec::new(),
            apply_default_triage_label: true,
        };

        let snapshot = syncer
            .create_issue_at(
                &repo,
                &request,
                IssueSyncAuthMode::Interactive,
                ts("2026-07-29T10:00:00Z"),
            )
            .await
            .unwrap();

        assert_eq!(
            snapshot.issue.issue_ref.to_string(),
            "mr-ples/agent-of-empires#21"
        );
        assert_eq!(
            syncer.client.mutation_calls(),
            vec!["create:mr-ples:Created issue:[\"needs-triage\"]"]
        );
        let persisted = syncer.store.load(&repo).unwrap().unwrap();
        assert_eq!(persisted.issues[0].title, "Created issue");
        assert_eq!(
            persisted.issues[0].sync.synced_at,
            Some(ts("2026-07-29T10:00:00Z"))
        );
        assert_eq!(
            syncer.client.auth_modes(),
            vec![IssueSyncAuthMode::Interactive]
        );
    }

    #[tokio::test]
    async fn refresh_issue_updates_only_the_affected_record() {
        let repo = repository();
        let issue_ref = IssueRef::new("Mr-Ples", "agent-of-empires", 14).unwrap();
        let (syncer, _temp) = mutation_syncer(vec![Ok(GitHubIssuePayload {
            title: "Refreshed issue".to_string(),
            ..payload(14, "Refreshed issue")
        })]);
        syncer
            .store
            .save(&IssueSyncCache {
                repository: repo.clone(),
                issues: vec![
                    payload(14, "Old issue")
                        .normalize(
                            &repo.owner,
                            &repo.repo,
                            IssueSyncMetadata::fresh(ts("2026-07-29T09:00:00Z")),
                        )
                        .unwrap(),
                    payload(15, "Other issue")
                        .normalize(
                            &repo.owner,
                            &repo.repo,
                            IssueSyncMetadata::fresh(ts("2026-07-29T09:00:00Z")),
                        )
                        .unwrap(),
                ],
                sync: IssueSyncMetadata::fresh(ts("2026-07-29T09:00:00Z")),
            })
            .unwrap();

        let snapshot = syncer
            .refresh_issue_at(
                &issue_ref,
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T10:00:00Z"),
            )
            .await
            .unwrap();

        assert_eq!(snapshot.issue.title, "Refreshed issue");
        assert_eq!(snapshot.cache.issues.len(), 2);
        assert_eq!(snapshot.cache.issues[1].title, "Other issue");
        assert_eq!(
            syncer.client.mutation_calls(),
            vec!["refresh:mr-ples/agent-of-empires#14"]
        );
    }

    #[tokio::test]
    async fn edit_issue_replaces_cached_record_without_touching_unsupported_fields() {
        let repo = repository();
        let issue_ref = IssueRef::new("Mr-Ples", "agent-of-empires", 14).unwrap();
        let (syncer, _temp) = mutation_syncer(vec![Ok(GitHubIssuePayload {
            title: "Edited issue".to_string(),
            body: Some("Edited body".to_string()),
            labels: vec![GitHubIssueLabelPayload {
                name: "p1".to_string(),
                color: None,
                description: None,
            }],
            ..payload(14, "Edited issue")
        })]);
        syncer
            .store
            .save(&IssueSyncCache {
                repository: repo.clone(),
                issues: vec![payload(14, "Old issue")
                    .normalize(
                        &repo.owner,
                        &repo.repo,
                        IssueSyncMetadata::fresh(ts("2026-07-29T09:00:00Z")),
                    )
                    .unwrap()],
                sync: IssueSyncMetadata::fresh(ts("2026-07-29T09:00:00Z")),
            })
            .unwrap();

        let snapshot = syncer
            .edit_issue_at(
                &issue_ref,
                &IssueEditRequest {
                    title: Some(" Edited issue ".to_string()),
                    body: Some(" Edited body ".to_string()),
                    labels: Some(vec!["p1".to_string()]),
                },
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T10:00:00Z"),
            )
            .await
            .unwrap();

        assert_eq!(snapshot.cache.issues.len(), 1);
        assert_eq!(snapshot.issue.title, "Edited issue");
        assert_eq!(snapshot.issue.body.as_deref(), Some("Edited body"));
        assert_eq!(
            syncer.client.mutation_calls(),
            vec!["edit:mr-ples/agent-of-empires#14:Some(\"Edited issue\"):Some([\"p1\"])"]
        );
    }

    #[tokio::test]
    async fn mutation_keeps_existing_repo_sync_metadata() {
        let repo = repository();
        let issue_ref = IssueRef::new("Mr-Ples", "agent-of-empires", 14).unwrap();
        let (syncer, _temp) = mutation_syncer(vec![Ok(GitHubIssuePayload {
            title: "Edited issue".to_string(),
            ..payload(14, "Edited issue")
        })]);
        syncer
            .store
            .save(&IssueSyncCache {
                repository: repo.clone(),
                issues: vec![payload(14, "Old issue")
                    .normalize(
                        &repo.owner,
                        &repo.repo,
                        IssueSyncMetadata::stale(IssueSyncFailureKind::Network, "offline"),
                    )
                    .unwrap()],
                sync: IssueSyncMetadata::stale(IssueSyncFailureKind::Network, "offline"),
            })
            .unwrap();

        let snapshot = syncer
            .edit_issue_at(
                &issue_ref,
                &IssueEditRequest {
                    title: Some("Edited issue".to_string()),
                    ..IssueEditRequest::default()
                },
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T10:00:00Z"),
            )
            .await
            .unwrap();

        assert_eq!(snapshot.issue.sync.status, IssueSyncStatus::Fresh);
        assert_eq!(snapshot.cache.sync.status, IssueSyncStatus::Stale);
        assert_eq!(
            snapshot.cache.sync.failure,
            Some(IssueSyncFailureKind::Network)
        );
    }

    #[tokio::test]
    async fn close_and_reopen_issue_update_cached_state() {
        let repo = repository();
        let issue_ref = IssueRef::new("Mr-Ples", "agent-of-empires", 14).unwrap();
        let (syncer, _temp) = mutation_syncer(vec![
            Ok(GitHubIssuePayload {
                state: "closed".to_string(),
                closed_at: Some(ts("2026-07-29T10:00:00Z")),
                ..payload(14, "Issue")
            }),
            Ok(GitHubIssuePayload {
                state: "open".to_string(),
                closed_at: None,
                ..payload(14, "Issue")
            }),
        ]);

        let closed = syncer
            .set_issue_state_at(
                &issue_ref,
                IssueState::Closed,
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T10:00:00Z"),
            )
            .await
            .unwrap();
        let reopened = syncer
            .set_issue_state_at(
                &issue_ref,
                IssueState::Open,
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T10:01:00Z"),
            )
            .await
            .unwrap();

        assert_eq!(closed.issue.state, IssueState::Closed);
        assert_eq!(reopened.issue.state, IssueState::Open);
        let persisted = syncer.store.load(&repo).unwrap().unwrap();
        assert_eq!(persisted.issues[0].state, IssueState::Open);
        assert_eq!(
            syncer.client.mutation_calls(),
            vec![
                "state:mr-ples/agent-of-empires#14:Closed",
                "state:mr-ples/agent-of-empires#14:Open"
            ]
        );
    }

    #[tokio::test]
    async fn failed_mutation_preserves_cached_issues_as_stale() {
        let repo = repository();
        let issue_ref = IssueRef::new("Mr-Ples", "agent-of-empires", 14).unwrap();
        let (syncer, _temp) = mutation_syncer(vec![Err(IssueSyncFailure::new(
            IssueSyncFailureKind::Network,
            "offline",
        ))]);
        syncer
            .store
            .save(&IssueSyncCache {
                repository: repo.clone(),
                issues: vec![payload(14, "Cached issue")
                    .normalize(
                        &repo.owner,
                        &repo.repo,
                        IssueSyncMetadata::fresh(ts("2026-07-29T09:00:00Z")),
                    )
                    .unwrap()],
                sync: IssueSyncMetadata::fresh(ts("2026-07-29T09:00:00Z")),
            })
            .unwrap();

        let err = syncer
            .edit_issue_at(
                &issue_ref,
                &IssueEditRequest {
                    title: Some("New title".to_string()),
                    ..IssueEditRequest::default()
                },
                IssueSyncAuthMode::NonInteractive,
                ts("2026-07-29T10:00:00Z"),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, IssueMutationError::GitHub { .. }));
        let persisted = syncer.store.load(&repo).unwrap().unwrap();
        assert_eq!(persisted.issues[0].title, "Cached issue");
        assert_eq!(persisted.sync.status, IssueSyncStatus::Stale);
        assert_eq!(persisted.issues[0].sync.status, IssueSyncStatus::Stale);
    }

    #[test]
    fn github_error_mapping_preserves_auth_mode() {
        let interactive = IssueSyncFailure::from_github_error(
            GitHubError::Unauthorized,
            IssueSyncAuthMode::Interactive,
        );
        let non_interactive = IssueSyncFailure::from_github_error(
            GitHubError::Unauthorized,
            IssueSyncAuthMode::NonInteractive,
        );

        assert_eq!(
            interactive.kind,
            IssueSyncFailureKind::AuthRequired { interactive: true }
        );
        assert_eq!(
            non_interactive.kind,
            IssueSyncFailureKind::AuthRequired { interactive: false }
        );
    }
}
