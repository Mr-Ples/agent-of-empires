//! GitHub Issue sync service and normalized cache storage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::github::{
    GitHubError, GitHubIssuePayload, IssueNormalizeError, IssueRecord, IssueSyncFailureKind,
    IssueSyncMetadata, IssueSyncStatus,
};

pub type IssueClientFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<GitHubIssuePayload>, IssueSyncFailure>> + Send + 'a>>;

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

pub trait GitHubIssueClient: Send + Sync {
    fn list_repo_issues<'a>(
        &'a self,
        repository: &'a IssueRepository,
        auth_mode: IssueSyncAuthMode,
    ) -> IssueClientFuture<'a>;
}

#[derive(Debug, Clone)]
pub struct IssueSyncStore {
    root: PathBuf,
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
        auth_modes: Mutex<Vec<IssueSyncAuthMode>>,
    }

    impl FakeGitHubIssueClient {
        fn new(responses: Vec<Result<Vec<GitHubIssuePayload>, IssueSyncFailure>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                auth_modes: Mutex::new(Vec::new()),
            }
        }

        fn auth_modes(&self) -> Vec<IssueSyncAuthMode> {
            self.auth_modes.lock().unwrap().clone()
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
