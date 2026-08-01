# GitHub Integration

Agent of Empires talks to GitHub through a single backend client (`src/github/`).
Every call to `api.github.com` goes through it. Public read workflows use an
unauthenticated client; issue mutation workflows use an authenticated client.
This page documents the typed failures that surface.

## When a request fails

Request failures are typed so the surface (a TUI toast or a web error banner)
can show the right next step:

- **401 Unauthorized**: the request is unauthenticated, the token was rejected,
  or the resource requires a signed-in client.
- **403 with a missing scope**: AoE names the required scope from GitHub's
  `X-Accepted-OAuth-Scopes` response header, for example `repo` or `workflow`.
  The current Global GitHub Credential cannot perform this operation.
- **403 or 429 rate limited**: wait for the limit to reset.
- **404 Not Found**: the resource does not exist or is not publicly visible.
- **Network unreachable**: distinguished from auth, so a GitHub outage never
  tells you to re-login.

## Issue ordering and search

The TUI Issues view and the web Issues sidebar use the issue preferences stored
on the project registry entry. These preferences are per project, not global
`config.toml` settings, so two repositories can use different issue workflows.

The registry entry supports these fields:

```json
{
  "name": "agent-of-empires",
  "path": "/work/agent-of-empires",
  "issue_sort_order": "github",
  "issue_label_priority": [
    "p0",
    "p1",
    "p2",
    "needs-triage",
    "ready-for-human",
    "needs-info",
    "ready-for-agent",
    "wontfix"
  ]
}
```

`issue_sort_order` is either:

- `github`, the default. Issues retain the order returned by GitHub.
- `label_priority`. Issues are grouped by the first matching label in
  `issue_label_priority`; labels earlier in the array have higher priority.

An issue with more than one configured label uses the first matching label in
the configured array. Issues with no matching label are placed after all
matched issues. Within the same priority, issue references provide a stable
tie-breaker.

Edit these values from the web dashboard's project editor. The TUI reads the
same project registry values, so a preference saved in the web dashboard is
used by the TUI as well. Global and profile project entries follow the normal
registry shadowing rules described above. Existing project files that do not
contain these fields use GitHub ordering and the default priority list shown in
the example.

Issue search is local to the currently selected project and matches the issue
title, issue reference, labels, and cached excerpt. An empty query shows all
issues. Search does not trigger another GitHub request, and it works on cached
issues while synchronization is unavailable.
