//! GitHub client foundation.
//!
//! One typed surface for talking to GitHub, shared by the TUI and the web
//! backend. The HTTP client and the error taxonomy live here so no other
//! module hits `api.github.com` directly.
//!
//! See `docs/github-integration.md` for the per-failure hints.

pub mod auth;
pub mod client;
pub mod error;
pub mod issues;
pub mod sync;

pub use auth::{recover_interactive, token as github_token, GitHubAuthError};
pub use client::{
    GitHubAsset, GitHubClient, GitHubClientConfig, GitHubCompare, GitHubCompareCommit,
    GitHubRelease, GitHubRepo,
};
pub use error::{GitHubError, Result};
pub use issues::{
    issue_context_prompt, issue_context_prompt_with_rules, issue_session_default_branch,
    issue_session_default_title, load_cached_issue_record, project_work_items,
    project_work_items_with_attention, sort_work_items, work_item_matches, work_item_priority,
    AttentionInputs, AttentionState, AttentionVisualTone, GitHubIssuePayload,
    IssueAttachmentConflict, IssueCreateRequest, IssueEditRequest, IssueLabel,
    IssueMutationValidationError, IssueNormalizeError, IssueRecord, IssueRef, IssueRefParseError,
    IssueSortOrder, IssueState, IssueSyncFailureKind, IssueSyncMetadata, IssueSyncStatus,
    LabelPromptRule, PullRequestBadge, RuntimeLiveness, ValidatedIssueCreateRequest,
    ValidatedIssueEditRequest, WorkItemListProjection, WorkItemProjection,
    WorkItemSessionAttachment, WorkItemState, DEFAULT_LABEL_PRIORITY, DEFAULT_TRIAGE_LABEL,
    label_prompt_instructions,
};
pub use sync::{
    issue_sync_cache_dir, GitHubIssueClient, IssueMutationError, IssueMutationSnapshot,
    IssueRepository, IssueSyncAuthMode, IssueSyncCache, IssueSyncError, IssueSyncFailure,
    IssueSyncSnapshot, IssueSyncStore, IssueSyncer,
};

/// Default GitHub REST API base.
pub const DEFAULT_GITHUB_API_BASE: &str = "https://api.github.com";
/// User-Agent sent on every GitHub request (GitHub requires one).
pub const DEFAULT_USER_AGENT: &str = "agent-of-empires";
