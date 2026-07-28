#!/usr/bin/env bash
set -euo pipefail

parent="${1:-11}"
repo="${2:-}"

usage() {
  cat <<'USAGE'
Usage: scripts/find-frontier.sh [parent-issue-number] [owner/repo]

Prints open, unblocked, unassigned sub-issues for a parent issue.

Examples:
  scripts/find-frontier.sh
  scripts/find-frontier.sh 11
  scripts/find-frontier.sh 11 Mr-Ples/agent-of-empires
USAGE
}

if [[ "${parent}" == "-h" || "${parent}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ -z "${repo}" ]]; then
  repo="$(gh repo view --json owner,name --jq '.owner.login + "/" + .name')"
fi

gh api "repos/${repo}/issues/${parent}/sub_issues" --paginate \
  --jq '.[] | select(.state == "open" and (.issue_dependencies_summary.blocked_by // 0) == 0 and ((.assignees // []) | length) == 0) | "#\(.number) \(.title)\n\(.html_url)"'
