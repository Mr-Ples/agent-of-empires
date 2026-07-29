//! Shared GitHub issue domain types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Stable issue identity, formatted as `owner/repo#number`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IssueRef {
    owner: String,
    repo: String,
    number: u64,
}

impl IssueRef {
    pub fn new(
        owner: impl AsRef<str>,
        repo: impl AsRef<str>,
        number: u64,
    ) -> Result<Self, IssueRefParseError> {
        let owner = normalize_owner(owner.as_ref())?;
        let repo = normalize_repo(repo.as_ref())?;
        if number == 0 {
            return Err(IssueRefParseError::InvalidNumber);
        }
        Ok(Self {
            owner,
            repo,
            number,
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn number(&self) -> u64 {
        self.number
    }
}

impl fmt::Display for IssueRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}#{}", self.owner, self.repo, self.number)
    }
}

impl FromStr for IssueRef {
    type Err = IssueRefParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim();
        let (repo_ref, number) = raw
            .rsplit_once('#')
            .ok_or(IssueRefParseError::MissingNumber)?;
        let (owner, repo) = repo_ref
            .split_once('/')
            .ok_or(IssueRefParseError::MissingRepo)?;
        if repo.contains('/') {
            return Err(IssueRefParseError::InvalidRepo);
        }
        let number = number
            .trim()
            .parse::<u64>()
            .map_err(|_| IssueRefParseError::InvalidNumber)?;
        Self::new(owner, repo, number)
    }
}

impl TryFrom<String> for IssueRef {
    type Error = IssueRefParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<IssueRef> for String {
    fn from(value: IssueRef) -> Self {
        value.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IssueRefParseError {
    #[error("issue ref must include an issue number after #")]
    MissingNumber,
    #[error("issue ref must include owner/repo before #")]
    MissingRepo,
    #[error("issue ref owner is invalid")]
    InvalidOwner,
    #[error("issue ref repo is invalid")]
    InvalidRepo,
    #[error("issue number must be a positive integer")]
    InvalidNumber,
}

#[derive(Debug, Error)]
pub enum IssueNormalizeError {
    #[error(transparent)]
    InvalidRef(#[from] IssueRefParseError),
    #[error("GitHub issue state is unsupported: {0}")]
    UnsupportedState(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueLabel {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestBadge {
    pub url: String,
    pub merged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSyncStatus {
    Fresh,
    Stale,
    AuthRequired,
    RateLimited,
    Forbidden,
    NotFound,
    Network,
    ApiFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSyncMetadata {
    pub status: IssueSyncStatus,
    pub synced_at: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

impl IssueSyncMetadata {
    pub fn fresh(synced_at: DateTime<Utc>) -> Self {
        Self {
            status: IssueSyncStatus::Fresh,
            synced_at: Some(synced_at),
            message: None,
        }
    }

    pub fn failure(status: IssueSyncStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            synced_at: None,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRecord {
    pub issue_ref: IssueRef,
    pub github_id: u64,
    pub node_id: String,
    pub title: String,
    pub body: Option<String>,
    pub excerpt: Option<String>,
    pub state: IssueState,
    pub labels: Vec<IssueLabel>,
    pub assignees: Vec<String>,
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub pull_request: Option<PullRequestBadge>,
    pub sync: IssueSyncMetadata,
}

impl IssueRecord {
    pub fn is_closed(&self) -> bool {
        self.state == IssueState::Closed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemProjection {
    pub issue_ref: IssueRef,
    pub title: String,
    pub state: WorkItemState,
    pub labels: Vec<IssueLabel>,
    pub url: String,
    pub pull_request: Option<PullRequestBadge>,
    pub sync: IssueSyncMetadata,
    pub issue: IssueRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemState {
    Open,
    Closed,
}

impl From<IssueRecord> for WorkItemProjection {
    fn from(issue: IssueRecord) -> Self {
        let state = match issue.state {
            IssueState::Open => WorkItemState::Open,
            IssueState::Closed => WorkItemState::Closed,
        };
        Self {
            issue_ref: issue.issue_ref.clone(),
            title: issue.title.clone(),
            state,
            labels: issue.labels.clone(),
            url: issue.url.clone(),
            pull_request: issue.pull_request.clone(),
            sync: issue.sync.clone(),
            issue,
        }
    }
}

/// GitHub REST issue payload subset normalized into [`IssueRecord`].
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubIssuePayload {
    pub id: u64,
    pub node_id: String,
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub labels: Vec<GitHubIssueLabelPayload>,
    #[serde(default)]
    pub assignees: Vec<GitHubIssueAssigneePayload>,
    pub html_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub pull_request: Option<GitHubPullRequestPayload>,
}

impl GitHubIssuePayload {
    pub fn normalize(
        self,
        owner: &str,
        repo: &str,
        sync: IssueSyncMetadata,
    ) -> Result<IssueRecord, IssueNormalizeError> {
        let issue_ref = IssueRef::new(owner, repo, self.number)?;
        let body = self.body.map(|body| body.trim().to_string());
        let excerpt = body.as_deref().and_then(excerpt);
        let state = match self.state.as_str() {
            "closed" => IssueState::Closed,
            "open" => IssueState::Open,
            other => return Err(IssueNormalizeError::UnsupportedState(other.to_string())),
        };

        Ok(IssueRecord {
            issue_ref,
            github_id: self.id,
            node_id: self.node_id,
            title: self.title.trim().to_string(),
            body,
            excerpt,
            state,
            labels: self
                .labels
                .into_iter()
                .map(|label| IssueLabel {
                    name: label.name,
                    color: normalize_color(label.color),
                    description: label.description,
                })
                .collect(),
            assignees: self
                .assignees
                .into_iter()
                .map(|assignee| assignee.login)
                .collect(),
            url: self.html_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
            pull_request: self.pull_request.map(|pr| PullRequestBadge {
                url: pr.html_url.unwrap_or(pr.url),
                merged_at: pr.merged_at,
            }),
            sync,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubIssueLabelPayload {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubIssueAssigneePayload {
    pub login: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubPullRequestPayload {
    pub url: String,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub merged_at: Option<DateTime<Utc>>,
}

fn normalize_owner(raw: &str) -> Result<String, IssueRefParseError> {
    let value = raw.trim();
    if value.is_empty()
        || value.starts_with('-')
        || value.ends_with('-')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(IssueRefParseError::InvalidOwner);
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_repo(raw: &str) -> Result<String, IssueRefParseError> {
    let value = raw.trim();
    if value.is_empty()
        || value.contains('/')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(IssueRefParseError::InvalidRepo);
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_color(color: Option<String>) -> Option<String> {
    color.and_then(|color| {
        let trimmed = color.trim().trim_start_matches('#');
        if trimmed.len() == 6 && trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Some(trimmed.to_ascii_lowercase())
        } else {
            None
        }
    })
}

fn excerpt(body: &str) -> Option<String> {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    const MAX_EXCERPT_CHARS: usize = 180;
    if collapsed.chars().count() <= MAX_EXCERPT_CHARS {
        return Some(collapsed);
    }
    let mut value = collapsed
        .chars()
        .take(MAX_EXCERPT_CHARS)
        .collect::<String>()
        .trim_end()
        .to_string();
    value.push_str("...");
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(raw: &str) -> DateTime<Utc> {
        raw.parse().unwrap()
    }

    fn issue_payload() -> GitHubIssuePayload {
        GitHubIssuePayload {
            id: 1001,
            node_id: "I_kwDO".to_string(),
            number: 12,
            title: "Issue title".to_string(),
            body: None,
            state: "open".to_string(),
            labels: Vec::new(),
            assignees: Vec::new(),
            html_url: "https://github.com/Mr-Ples/agent-of-empires/issues/12".to_string(),
            created_at: ts("2026-07-01T12:00:00Z"),
            updated_at: ts("2026-07-02T12:00:00Z"),
            closed_at: None,
            pull_request: None,
        }
    }

    #[test]
    fn issue_ref_parses_formats_and_normalizes_identity() {
        let issue_ref: IssueRef = "  Mr-Ples/Agent_Of.Empires#42 ".parse().unwrap();

        assert_eq!(issue_ref.owner(), "mr-ples");
        assert_eq!(issue_ref.repo(), "agent_of.empires");
        assert_eq!(issue_ref.number(), 42);
        assert_eq!(issue_ref.to_string(), "mr-ples/agent_of.empires#42");
        assert_eq!(
            issue_ref,
            IssueRef::new("MR-PLES", "Agent_Of.Empires", 42).unwrap()
        );
    }

    #[test]
    fn issue_ref_rejects_invalid_refs() {
        for raw in [
            "",
            "owner/repo",
            "owner#1",
            "owner/repo#0",
            "owner/repo#nope",
            "/repo#1",
            "owner/#1",
            "owner/repo/extra#1",
            "bad owner/repo#1",
            "-owner/repo#1",
        ] {
            assert!(raw.parse::<IssueRef>().is_err(), "{raw} should be invalid");
        }
    }

    #[test]
    fn github_payload_normalizes_to_issue_record() {
        let payload = GitHubIssuePayload {
            title: "  Add shared model  ".to_string(),
            body: Some("First line.\n\nSecond line with more detail.".to_string()),
            state: "closed".to_string(),
            labels: vec![
                GitHubIssueLabelPayload {
                    name: "ready-for-agent".to_string(),
                    color: Some("#0E8A16".to_string()),
                    description: Some("Ready".to_string()),
                },
                GitHubIssueLabelPayload {
                    name: "bad-color".to_string(),
                    color: Some("not-a-color".to_string()),
                    description: None,
                },
            ],
            assignees: vec![GitHubIssueAssigneePayload {
                login: "simon".to_string(),
            }],
            closed_at: Some(ts("2026-07-03T12:00:00Z")),
            pull_request: Some(GitHubPullRequestPayload {
                url: "https://api.github.com/repos/Mr-Ples/agent-of-empires/pulls/12".to_string(),
                html_url: Some("https://github.com/Mr-Ples/agent-of-empires/pull/12".to_string()),
                merged_at: Some(ts("2026-07-03T12:00:00Z")),
            }),
            ..issue_payload()
        };

        let record = payload
            .normalize(
                "Mr-Ples",
                "Agent-Of-Empires",
                IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
            )
            .unwrap();

        assert_eq!(record.issue_ref.to_string(), "mr-ples/agent-of-empires#12");
        assert_eq!(record.title, "Add shared model");
        assert_eq!(
            record.excerpt.as_deref(),
            Some("First line. Second line with more detail.")
        );
        assert_eq!(record.state, IssueState::Closed);
        assert!(record.is_closed());
        assert_eq!(record.labels[0].color.as_deref(), Some("0e8a16"));
        assert_eq!(record.labels[1].color, None);
        assert_eq!(record.assignees, vec!["simon"]);
        assert_eq!(
            record.pull_request.as_ref().map(|pr| pr.url.as_str()),
            Some("https://github.com/Mr-Ples/agent-of-empires/pull/12")
        );
        assert_eq!(record.sync.status, IssueSyncStatus::Fresh);
    }

    #[test]
    fn work_item_projection_marks_closed_issues() {
        let payload = GitHubIssuePayload {
            title: "Closed issue".to_string(),
            state: "closed".to_string(),
            closed_at: Some(ts("2026-07-03T12:00:00Z")),
            ..issue_payload()
        };
        let record = payload
            .normalize(
                "Mr-Ples",
                "agent-of-empires",
                IssueSyncMetadata::failure(IssueSyncStatus::Stale, "network unavailable"),
            )
            .unwrap();

        let work_item = WorkItemProjection::from(record);

        assert_eq!(work_item.state, WorkItemState::Closed);
        assert_eq!(
            work_item.issue_ref.to_string(),
            "mr-ples/agent-of-empires#12"
        );
        assert_eq!(work_item.title, "Closed issue");
        assert_eq!(
            work_item.url,
            "https://github.com/Mr-Ples/agent-of-empires/issues/12"
        );
        assert_eq!(work_item.issue.sync.status, IssueSyncStatus::Stale);
        assert_eq!(
            work_item.issue.sync.message.as_deref(),
            Some("network unavailable")
        );
    }

    #[test]
    fn github_payload_rejects_unknown_issue_state() {
        let payload = GitHubIssuePayload {
            title: "Issue with new state".to_string(),
            state: "archived".to_string(),
            ..issue_payload()
        };

        let err = payload
            .normalize(
                "Mr-Ples",
                "agent-of-empires",
                IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
            )
            .unwrap_err();

        assert!(matches!(err, IssueNormalizeError::UnsupportedState(state) if state == "archived"));
    }
}
