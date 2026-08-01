//! Shared GitHub credential lookup and explicit interactive recovery.

use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitHubAuthError {
    #[error("GitHub authentication requires the gh CLI: {0}")]
    MissingCli(#[source] std::io::Error),
    #[error("GitHub authentication did not complete successfully")]
    Failed,
}

/// Find a credential without prompting or opening anything.
pub fn token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
        .or_else(|| {
            let output = Command::new("gh")
                .args(["auth", "token"])
                .stderr(Stdio::null())
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
}

/// Start the GitHub CLI's browser-based device login. This is intentionally
/// separate from [`token`], so non-interactive issue workflows never prompt.
pub fn recover_interactive() -> Result<(), GitHubAuthError> {
    let status = Command::new("gh")
        .args(["auth", "login", "--web", "--hostname", "github.com"])
        .status()
        .map_err(GitHubAuthError::MissingCli)?;
    status
        .success()
        .then_some(())
        .ok_or(GitHubAuthError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credentials_are_non_interactive() {
        // The lookup has no recovery side effect; callers decide whether to
        // offer recover_interactive based on their interaction mode.
        let _ = token();
    }
}
