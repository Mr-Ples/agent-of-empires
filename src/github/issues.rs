//! Shared GitHub issue domain types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

pub const DEFAULT_TRIAGE_LABEL: &str = "needs-triage";

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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IssueMutationValidationError {
    #[error("issue title is required")]
    MissingTitle,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{issue_ref} is already attached to session {holder_session_id}")]
pub struct IssueAttachmentConflict {
    pub issue_ref: IssueRef,
    pub holder_session_id: String,
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
pub struct IssueCreateRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default)]
    pub apply_default_triage_label: bool,
}

impl IssueCreateRequest {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            labels: Vec::new(),
            apply_default_triage_label: false,
        }
    }

    pub fn validated(&self) -> Result<ValidatedIssueCreateRequest, IssueMutationValidationError> {
        let title = normalize_required_title(&self.title)?;
        let body = self.body.as_ref().map(|body| body.trim().to_string());
        let labels = normalize_labels(&self.labels, self.apply_default_triage_label);
        Ok(ValidatedIssueCreateRequest {
            title,
            body,
            labels,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedIssueCreateRequest {
    pub title: String,
    pub body: Option<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IssueEditRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

impl IssueEditRequest {
    pub fn validated(&self) -> Result<ValidatedIssueEditRequest, IssueMutationValidationError> {
        let title = self
            .title
            .as_ref()
            .map(|title| normalize_required_title(title))
            .transpose()?;
        let body = self.body.as_ref().map(|body| body.trim().to_string());
        let labels = self
            .labels
            .as_ref()
            .map(|labels| normalize_labels(labels, false));
        Ok(ValidatedIssueEditRequest {
            title,
            body,
            labels,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedIssueEditRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestBadge {
    pub url: String,
    pub merged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<IssueSyncFailureKind>,
}

impl IssueSyncMetadata {
    pub fn fresh(synced_at: DateTime<Utc>) -> Self {
        Self {
            status: IssueSyncStatus::Fresh,
            synced_at: Some(synced_at),
            message: None,
            failure: None,
        }
    }

    pub fn failure(status: IssueSyncStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            synced_at: None,
            message: Some(message.into()),
            failure: status.failure_kind(),
        }
    }

    pub fn stale(failure: IssueSyncFailureKind, message: impl Into<String>) -> Self {
        Self {
            status: IssueSyncStatus::Stale,
            synced_at: None,
            message: Some(message.into()),
            failure: Some(failure),
        }
    }
}

impl IssueSyncStatus {
    pub fn failure_kind(self) -> Option<IssueSyncFailureKind> {
        match self {
            IssueSyncStatus::Fresh | IssueSyncStatus::Stale => None,
            IssueSyncStatus::AuthRequired => {
                Some(IssueSyncFailureKind::AuthRequired { interactive: false })
            }
            IssueSyncStatus::RateLimited => Some(IssueSyncFailureKind::RateLimited),
            IssueSyncStatus::Forbidden => Some(IssueSyncFailureKind::Forbidden),
            IssueSyncStatus::NotFound => Some(IssueSyncFailureKind::NotFound),
            IssueSyncStatus::Network => Some(IssueSyncFailureKind::Network),
            IssueSyncStatus::ApiFailure => Some(IssueSyncFailureKind::ApiFailure),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IssueSyncFailureKind {
    AuthRequired { interactive: bool },
    Forbidden,
    NotFound,
    Network,
    RateLimited,
    ApiFailure,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_liveness: Option<RuntimeLiveness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_state: Option<AttentionState>,
    pub labels: Vec<IssueLabel>,
    pub url: String,
    pub pull_request: Option<PullRequestBadge>,
    pub sync: IssueSyncMetadata,
    pub issue: IssueRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemListProjection {
    pub open: Vec<WorkItemProjection>,
    pub closed: Vec<WorkItemProjection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLiveness {
    Active,
    Idle,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionState {
    NeedsInput,
    Error,
    Idle,
    Active,
    Stopped,
}

impl AttentionState {
    pub fn priority(self) -> u8 {
        match self {
            AttentionState::NeedsInput => 0,
            AttentionState::Error => 1,
            AttentionState::Idle => 2,
            AttentionState::Active => 3,
            AttentionState::Stopped => 4,
        }
    }

    pub fn visual_tone(self) -> AttentionVisualTone {
        match self {
            AttentionState::NeedsInput => AttentionVisualTone::NeedsInput,
            AttentionState::Error => AttentionVisualTone::Error,
            AttentionState::Idle => AttentionVisualTone::Idle,
            AttentionState::Active => AttentionVisualTone::Active,
            AttentionState::Stopped => AttentionVisualTone::Stopped,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionVisualTone {
    NeedsInput,
    Error,
    Idle,
    Active,
    Stopped,
}

impl AttentionVisualTone {
    pub fn is_red(self) -> bool {
        self == AttentionVisualTone::Error
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttentionInputs {
    pub runtime_liveness: RuntimeLiveness,
    pub lifecycle_needs_input: bool,
    pub structured_needs_input: bool,
}

impl AttentionInputs {
    pub fn new(runtime_liveness: RuntimeLiveness) -> Self {
        Self {
            runtime_liveness,
            lifecycle_needs_input: false,
            structured_needs_input: false,
        }
    }

    pub fn compute(self) -> AttentionState {
        if self.lifecycle_needs_input || self.structured_needs_input {
            return AttentionState::NeedsInput;
        }

        match self.runtime_liveness {
            RuntimeLiveness::Error => AttentionState::Error,
            RuntimeLiveness::Idle => AttentionState::Idle,
            RuntimeLiveness::Active => AttentionState::Active,
            RuntimeLiveness::Stopped => AttentionState::Stopped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItemSessionAttachment {
    pub issue_ref: IssueRef,
    pub session_id: String,
    pub attention: AttentionInputs,
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
            attached_session_id: None,
            runtime_liveness: None,
            attention_state: None,
            labels: issue.labels.clone(),
            url: issue.url.clone(),
            pull_request: issue.pull_request.clone(),
            sync: issue.sync.clone(),
            issue,
        }
    }
}

pub fn project_work_items<'a>(
    issues: impl IntoIterator<Item = IssueRecord>,
    attachments: impl IntoIterator<Item = (&'a IssueRef, &'a str)>,
) -> WorkItemListProjection {
    let attached_by_issue: HashMap<IssueRef, String> = attachments
        .into_iter()
        .map(|(issue_ref, session_id)| (issue_ref.clone(), session_id.to_string()))
        .collect();

    let mut open = Vec::new();
    let mut closed = Vec::new();
    for issue in issues {
        let mut work_item = WorkItemProjection::from(issue);
        work_item.attached_session_id = attached_by_issue.get(&work_item.issue_ref).cloned();
        match work_item.state {
            WorkItemState::Open => open.push(work_item),
            WorkItemState::Closed => closed.push(work_item),
        }
    }

    WorkItemListProjection { open, closed }
}

pub fn project_work_items_with_attention(
    issues: impl IntoIterator<Item = IssueRecord>,
    attachments: impl IntoIterator<Item = WorkItemSessionAttachment>,
) -> WorkItemListProjection {
    let attached_by_issue: HashMap<IssueRef, WorkItemSessionAttachment> = attachments
        .into_iter()
        .map(|attachment| (attachment.issue_ref.clone(), attachment))
        .collect();

    let mut open = Vec::new();
    let mut closed = Vec::new();
    for issue in issues {
        let mut work_item = WorkItemProjection::from(issue);
        if let Some(attachment) = attached_by_issue.get(&work_item.issue_ref) {
            work_item.attached_session_id = Some(attachment.session_id.clone());
            work_item.runtime_liveness = Some(attachment.attention.runtime_liveness);
            work_item.attention_state = Some(attachment.attention.compute());
        }
        match work_item.state {
            WorkItemState::Open => open.push(work_item),
            WorkItemState::Closed => closed.push(work_item),
        }
    }

    WorkItemListProjection { open, closed }
}

pub fn issue_session_default_title(issue_ref: &IssueRef, issue: Option<&IssueRecord>) -> String {
    let normalized_title = issue
        .map(|issue| issue.title.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|title| !title.is_empty());
    match normalized_title {
        Some(title) => format!("#{} {title}", issue_ref.number()),
        None => format!("#{} {}", issue_ref.number(), issue_ref),
    }
}

pub fn issue_session_default_branch(issue_ref: &IssueRef) -> String {
    format!(
        "issue-{}-{}-{}",
        issue_ref.owner(),
        issue_ref.repo(),
        issue_ref.number()
    )
}

pub fn issue_context_prompt(issue_ref: &IssueRef, issue: Option<&IssueRecord>) -> String {
    let mut lines = vec![
        "Issue Context".to_string(),
        String::new(),
        format!("Issue Ref: {issue_ref}"),
    ];

    if let Some(issue) = issue {
        lines.push(format!("Title: {}", issue.title));
        lines.push(format!("URL: {}", issue.url));
        if !issue.labels.is_empty() {
            let labels = issue
                .labels
                .iter()
                .map(|label| label.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("Labels: {labels}"));
        }
        if let Some(body) = issue
            .body
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            lines.push(String::new());
            lines.push("Body:".to_string());
            lines.push(body.to_string());
        }
    } else {
        lines.push("Title: unavailable in local issue cache".to_string());
    }

    lines.join("\n")
}

pub fn load_cached_issue_record(
    app_dir: impl Into<std::path::PathBuf>,
    issue_ref: &IssueRef,
) -> Option<IssueRecord> {
    let repository =
        crate::github::IssueRepository::new(issue_ref.owner(), issue_ref.repo()).ok()?;
    let store =
        crate::github::IssueSyncStore::new(crate::github::issue_sync_cache_dir(app_dir.into()));
    let cache = store.load(&repository).ok().flatten()?;
    cache
        .issues
        .into_iter()
        .find(|issue| issue.issue_ref == *issue_ref)
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

fn normalize_required_title(raw: &str) -> Result<String, IssueMutationValidationError> {
    let title = raw.trim().to_string();
    if title.is_empty() {
        return Err(IssueMutationValidationError::MissingTitle);
    }
    Ok(title)
}

fn normalize_labels(labels: &[String], apply_default_triage_label: bool) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    if apply_default_triage_label {
        normalized.push(DEFAULT_TRIAGE_LABEL.to_string());
        seen.insert(DEFAULT_TRIAGE_LABEL.to_string());
    }
    for label in labels.iter().map(String::as_str) {
        let label = label.trim();
        if label.is_empty() {
            continue;
        }
        let key = label.to_ascii_lowercase();
        if seen.insert(key) {
            normalized.push(label.to_string());
        }
    }
    normalized
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
    fn create_request_requires_title_and_applies_default_triage_label() {
        let request = IssueCreateRequest {
            title: "  New issue  ".to_string(),
            body: Some("  Body  ".to_string()),
            labels: vec![
                "ready-for-agent".to_string(),
                "Needs-Triage".to_string(),
                "needs-triage".to_string(),
                " ".to_string(),
            ],
            apply_default_triage_label: true,
        };

        let validated = request.validated().unwrap();

        assert_eq!(validated.title, "New issue");
        assert_eq!(validated.body.as_deref(), Some("Body"));
        assert_eq!(validated.labels, vec!["needs-triage", "ready-for-agent"]);
        assert!(IssueCreateRequest::new(" ").validated().is_err());
    }

    #[test]
    fn edit_request_validates_optional_title_and_replaces_labels() {
        let request = IssueEditRequest {
            title: Some("  Updated  ".to_string()),
            body: Some(" Updated body ".to_string()),
            labels: Some(vec!["p1".to_string(), "P1".to_string(), "".to_string()]),
        };

        let validated = request.validated().unwrap();

        assert_eq!(validated.title.as_deref(), Some("Updated"));
        assert_eq!(validated.body.as_deref(), Some("Updated body"));
        assert_eq!(validated.labels, Some(vec!["p1".to_string()]));
        assert!(IssueEditRequest {
            title: Some(" ".to_string()),
            ..IssueEditRequest::default()
        }
        .validated()
        .is_err());
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
    fn work_item_list_joins_session_attachments_and_keeps_unattached_rows() {
        let open_attached = GitHubIssuePayload {
            number: 12,
            title: "Attached issue".to_string(),
            ..issue_payload()
        }
        .normalize(
            "Mr-Ples",
            "agent-of-empires",
            IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
        )
        .unwrap();
        let open_unattached = GitHubIssuePayload {
            id: 1002,
            node_id: "I_other".to_string(),
            number: 13,
            title: "Unattached issue".to_string(),
            html_url: "https://github.com/Mr-Ples/agent-of-empires/issues/13".to_string(),
            ..issue_payload()
        }
        .normalize(
            "Mr-Ples",
            "agent-of-empires",
            IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
        )
        .unwrap();
        let closed_attached = GitHubIssuePayload {
            id: 1003,
            node_id: "I_closed".to_string(),
            number: 14,
            title: "Closed attached".to_string(),
            state: "closed".to_string(),
            html_url: "https://github.com/Mr-Ples/agent-of-empires/issues/14".to_string(),
            closed_at: Some(ts("2026-07-03T12:00:00Z")),
            ..issue_payload()
        }
        .normalize(
            "Mr-Ples",
            "agent-of-empires",
            IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
        )
        .unwrap();

        let projection = project_work_items(
            vec![
                open_attached.clone(),
                open_unattached.clone(),
                closed_attached.clone(),
            ],
            [
                (&open_attached.issue_ref, "session-a"),
                (&closed_attached.issue_ref, "session-c"),
            ],
        );

        assert_eq!(projection.open.len(), 2);
        assert_eq!(projection.closed.len(), 1);
        assert_eq!(
            projection.open[0].attached_session_id.as_deref(),
            Some("session-a")
        );
        assert_eq!(projection.open[1].title, "Unattached issue");
        assert_eq!(projection.open[1].attached_session_id, None);
        assert_eq!(
            projection.closed[0].attached_session_id.as_deref(),
            Some("session-c")
        );

        let detached_projection = project_work_items(
            vec![open_attached.clone()],
            std::iter::empty::<(&IssueRef, &str)>(),
        );
        assert_eq!(detached_projection.open.len(), 1);
        assert_eq!(
            detached_projection.open[0].issue_ref,
            open_attached.issue_ref
        );
        assert_eq!(detached_projection.open[0].attached_session_id, None);
    }

    #[test]
    fn attention_inputs_compute_issue_priority_without_lifecycle_status_changes() {
        assert_eq!(
            AttentionInputs {
                runtime_liveness: RuntimeLiveness::Error,
                lifecycle_needs_input: true,
                structured_needs_input: false,
            }
            .compute(),
            AttentionState::NeedsInput
        );
        assert_eq!(
            AttentionInputs {
                runtime_liveness: RuntimeLiveness::Error,
                lifecycle_needs_input: false,
                structured_needs_input: true,
            }
            .compute(),
            AttentionState::NeedsInput
        );
        assert_eq!(
            AttentionInputs::new(RuntimeLiveness::Error).compute(),
            AttentionState::Error
        );
        assert_eq!(
            AttentionInputs::new(RuntimeLiveness::Idle).compute(),
            AttentionState::Idle
        );
        assert_eq!(
            AttentionInputs::new(RuntimeLiveness::Active).compute(),
            AttentionState::Active
        );
        assert_eq!(
            AttentionInputs::new(RuntimeLiveness::Stopped).compute(),
            AttentionState::Stopped
        );

        assert_eq!(
            [
                AttentionState::NeedsInput,
                AttentionState::Error,
                AttentionState::Idle,
                AttentionState::Active,
                AttentionState::Stopped,
            ]
            .map(AttentionState::priority),
            [0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn attention_visual_tone_reserves_red_for_errors() {
        for state in [
            AttentionState::NeedsInput,
            AttentionState::Idle,
            AttentionState::Active,
            AttentionState::Stopped,
        ] {
            assert!(
                !state.visual_tone().is_red(),
                "{state:?} must not use the error tone"
            );
        }
        assert!(AttentionState::Error.visual_tone().is_red());
    }

    #[test]
    fn attention_projection_requires_an_attached_session() {
        let attached = GitHubIssuePayload {
            number: 12,
            title: "Attached issue".to_string(),
            ..issue_payload()
        }
        .normalize(
            "Mr-Ples",
            "agent-of-empires",
            IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
        )
        .unwrap();
        let unattached = GitHubIssuePayload {
            id: 1002,
            node_id: "I_other".to_string(),
            number: 13,
            title: "Unattached issue".to_string(),
            html_url: "https://github.com/Mr-Ples/agent-of-empires/issues/13".to_string(),
            ..issue_payload()
        }
        .normalize(
            "Mr-Ples",
            "agent-of-empires",
            IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
        )
        .unwrap();

        let projection = project_work_items_with_attention(
            vec![attached.clone(), unattached],
            [WorkItemSessionAttachment {
                issue_ref: attached.issue_ref.clone(),
                session_id: "session-a".to_string(),
                attention: AttentionInputs {
                    runtime_liveness: RuntimeLiveness::Idle,
                    lifecycle_needs_input: false,
                    structured_needs_input: true,
                },
            }],
        );

        assert_eq!(
            projection.open[0].attached_session_id.as_deref(),
            Some("session-a")
        );
        assert_eq!(
            projection.open[0].runtime_liveness,
            Some(RuntimeLiveness::Idle)
        );
        assert_eq!(
            projection.open[0].attention_state,
            Some(AttentionState::NeedsInput)
        );
        assert_eq!(projection.open[1].attached_session_id, None);
        assert_eq!(projection.open[1].runtime_liveness, None);
        assert_eq!(projection.open[1].attention_state, None);
    }

    #[test]
    fn issue_session_defaults_use_stable_issue_identity() {
        let record = GitHubIssuePayload {
            number: 17,
            title: " Support issue-first session creation\nand attach ".to_string(),
            html_url: "https://github.com/Mr-Ples/agent-of-empires/issues/17".to_string(),
            ..issue_payload()
        }
        .normalize(
            "Mr-Ples",
            "agent-of-empires",
            IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
        )
        .unwrap();

        assert_eq!(
            issue_session_default_title(&record.issue_ref, Some(&record)),
            "#17 Support issue-first session creation and attach"
        );
        assert_eq!(
            issue_session_default_branch(&record.issue_ref),
            "issue-mr-ples-agent-of-empires-17"
        );
    }

    #[test]
    fn issue_context_prompt_includes_cached_issue_details() {
        let record = GitHubIssuePayload {
            number: 17,
            title: "Support issue-first session creation".to_string(),
            body: Some("Acceptance criteria here.".to_string()),
            labels: vec![GitHubIssueLabelPayload {
                name: "ready-for-agent".to_string(),
                color: None,
                description: None,
            }],
            html_url: "https://github.com/Mr-Ples/agent-of-empires/issues/17".to_string(),
            ..issue_payload()
        }
        .normalize(
            "Mr-Ples",
            "agent-of-empires",
            IssueSyncMetadata::fresh(ts("2026-07-04T12:00:00Z")),
        )
        .unwrap();

        let prompt = issue_context_prompt(&record.issue_ref, Some(&record));

        assert!(prompt.contains("Issue Ref: mr-ples/agent-of-empires#17"));
        assert!(prompt.contains("Title: Support issue-first session creation"));
        assert!(prompt.contains("Labels: ready-for-agent"));
        assert!(prompt.contains("Acceptance criteria here."));
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
