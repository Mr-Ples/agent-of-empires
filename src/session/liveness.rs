use chrono::{DateTime, Utc};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::session::config::{WorkItemPromptPatternConfig, WorkItemsConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeLiveness {
    Active,
    Idle,
    Stopped,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivenessObservation {
    pub runtime_liveness: SessionRuntimeLiveness,
    pub output_hash: String,
    pub changed_at: DateTime<Utc>,
    pub needs_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromptPattern {
    id: &'static str,
    agent: &'static str,
    pattern: &'static str,
}

const BUILT_IN_PROMPT_PATTERNS: &[PromptPattern] = &[
    PromptPattern {
        id: "approval-question",
        agent: "claude",
        pattern: r"\b(do you want to|would you like to proceed)\b",
    },
    PromptPattern {
        id: "ask-user-question",
        agent: "claude",
        pattern: r"\benter to select\b.*\b(to navigate|esc to cancel)\b",
    },
    PromptPattern {
        id: "codex-approval",
        agent: "codex",
        pattern: r"\b(approve|allow|permission request|enter to approve)\b",
    },
    PromptPattern {
        id: "generic-yes-no",
        agent: "",
        pattern: r"(\(y/n\)|\[y/n\]|\byes/no\b)",
    },
];

pub fn observe_pane_text(
    content: &str,
    agent: &str,
    config: &WorkItemsConfig,
    previous_hash: Option<&str>,
    previous_changed_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> LivenessObservation {
    let normalized = normalize_visible_text(content);
    let output_hash = hash_visible_text(&normalized);
    let changed = previous_hash != Some(output_hash.as_str());
    let changed_at = if changed {
        now
    } else {
        previous_changed_at.unwrap_or(now)
    };
    let idle_after = chrono::Duration::seconds(config.liveness_idle_after_secs.max(1) as i64);
    let runtime_liveness = if changed || now.signed_duration_since(changed_at) < idle_after {
        SessionRuntimeLiveness::Active
    } else {
        SessionRuntimeLiveness::Idle
    };

    LivenessObservation {
        runtime_liveness,
        output_hash,
        changed_at,
        needs_input: runtime_liveness == SessionRuntimeLiveness::Idle
            && prompt_matches(&normalized, agent, config),
    }
}

fn normalize_visible_text(content: &str) -> String {
    crate::tmux::utils::strip_ansi(content).replace("\r\n", "\n")
}

fn hash_visible_text(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn prompt_matches(content: &str, agent: &str, config: &WorkItemsConfig) -> bool {
    let disabled = |pattern_agent: &str, id: &str| {
        config
            .disabled_built_in_prompt_patterns
            .get(id)
            .copied()
            .unwrap_or(false)
            || config
                .disabled_built_in_prompt_patterns
                .get(&format!("{pattern_agent}:{id}"))
                .copied()
                .unwrap_or(false)
    };

    for built_in in BUILT_IN_PROMPT_PATTERNS {
        if !built_in.agent.is_empty() && built_in.agent != agent {
            continue;
        }
        if disabled(built_in.agent, built_in.id) {
            continue;
        }
        if pattern_matches(content, built_in.pattern) {
            return true;
        }
    }

    config
        .prompt_patterns
        .values()
        .any(|custom| custom_applies(custom, agent) && pattern_matches(content, &custom.pattern))
}

fn custom_applies(pattern: &WorkItemPromptPatternConfig, agent: &str) -> bool {
    pattern.agent.is_empty() || pattern.agent == agent
}

fn pattern_matches(content: &str, pattern: &str) -> bool {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .dot_matches_new_line(true)
        .build()
        .map(|re| re.is_match(content))
        .unwrap_or_else(|e| {
            tracing::warn!(target: "session.liveness", pattern, "invalid prompt detector pattern: {e}");
            false
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_change_marks_active_and_resets_changed_at() {
        let config = WorkItemsConfig::default();
        let previous = "old";
        let now = DateTime::parse_from_rfc3339("2026-01-01T00:00:10Z")
            .unwrap()
            .with_timezone(&Utc);

        let observed = observe_pane_text("new", "claude", &config, Some(previous), None, now);

        assert_eq!(observed.runtime_liveness, SessionRuntimeLiveness::Active);
        assert_eq!(observed.changed_at, now);
    }

    #[test]
    fn unchanged_output_becomes_idle_after_threshold() {
        let config = WorkItemsConfig::default();
        let first = observe_pane_text(
            "same",
            "claude",
            &config,
            None,
            None,
            DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        );

        let second = observe_pane_text(
            "same",
            "claude",
            &config,
            Some(&first.output_hash),
            Some(first.changed_at),
            DateTime::parse_from_rfc3339("2026-01-01T00:00:05Z")
                .unwrap()
                .with_timezone(&Utc),
        );

        assert_eq!(second.runtime_liveness, SessionRuntimeLiveness::Idle);
    }

    #[test]
    fn prompt_detection_is_case_insensitive_and_can_disable_built_ins() {
        let mut config = WorkItemsConfig::default();
        let now = DateTime::parse_from_rfc3339("2026-01-01T00:00:10Z")
            .unwrap()
            .with_timezone(&Utc);
        let first = observe_pane_text(
            "Would You Like To Proceed?",
            "claude",
            &config,
            None,
            None,
            now,
        );
        assert_eq!(first.runtime_liveness, SessionRuntimeLiveness::Active);
        assert!(!first.needs_input);

        assert!(
            observe_pane_text(
                "Would You Like To Proceed?",
                "claude",
                &config,
                Some(&first.output_hash),
                Some(now - chrono::Duration::seconds(5)),
                now
            )
            .needs_input
        );

        config
            .disabled_built_in_prompt_patterns
            .insert("claude:approval-question".to_string(), true);
        assert!(
            !observe_pane_text(
                "Would You Like To Proceed?",
                "claude",
                &config,
                Some(&first.output_hash),
                Some(now - chrono::Duration::seconds(5)),
                now
            )
            .needs_input
        );
    }

    #[test]
    fn custom_patterns_respect_agent_scope() {
        let mut config = WorkItemsConfig::default();
        config.prompt_patterns.insert(
            "custom-block".to_string(),
            WorkItemPromptPatternConfig {
                agent: "opencode".to_string(),
                pattern: "choose an option".to_string(),
            },
        );
        let now = DateTime::parse_from_rfc3339("2026-01-01T00:00:10Z")
            .unwrap()
            .with_timezone(&Utc);
        let first = observe_pane_text("Choose an option", "opencode", &config, None, None, now);

        assert!(
            observe_pane_text(
                "Choose an option",
                "opencode",
                &config,
                Some(&first.output_hash),
                Some(now - chrono::Duration::seconds(5)),
                now
            )
            .needs_input
        );
        assert!(
            !observe_pane_text(
                "Choose an option",
                "claude",
                &config,
                None,
                None,
                Utc::now()
            )
            .needs_input
        );
    }
}
