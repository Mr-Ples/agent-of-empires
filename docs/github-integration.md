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
