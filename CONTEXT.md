# Domain Glossary

## Work Item

A GitHub Issue represented as a first-class AoE object. A Work Item can exist
and render without any AoE Session attached to it.

## Issue Ref

A stable reference to a GitHub Issue, written as `owner/repo#number`.

## Session Attachment

An optional one-to-one link between an AoE Session and a Work Item. The Work
Item owns issue state; the Session contributes runtime and liveness state when
attached. A Work Item may have no attached Session, and a Session may have no
attached Work Item.

## Issue-Created Session

An AoE Session created from a Work Item's `n` flow. It starts with a Session
Attachment, issue-derived defaults, and issue context injection enabled unless
the user turns it off during creation.

## Runtime Liveness

The user-facing activity state observed from an attached Session's runtime.
Runtime Liveness is persisted with the Session and projected onto an attached
Work Item for issue attention surfaces such as dots, attention navigation, and
notifications; lifecycle `Status` remains separate.
_Avoid_: Permission status, session status

## Attention State

The highest-priority user-facing state computed from lifecycle `Status`,
Runtime Liveness, and structured agent signals. Issue views render Attention
State when a Work Item has an attached Session.
_Avoid_: Status

## Needs Input

A Runtime Liveness or Attention State meaning the attached runtime appears to
be waiting for a human response, such as permission, approval, or
clarification.
_Avoid_: Needs permission

## Issue Record

AoE's normalized local projection of a GitHub Issue, shaped for shared TUI and
web dashboard consumption. GitHub-specific sync metadata stays at the edge of
the record.

## Issue Label

Display and filtering metadata copied from GitHub onto an Issue Record. Labels
do not define Work Item lifecycle state in v1.

## Label Prompt Rule

AoE-owned configuration that matches an Issue Label and contributes startup
instructions when issue context injection is enabled for a new Session.

## Issue Context

Optional startup context injected into a new Session from an attached Work
Item. It can include the Issue Ref, title, body, labels, URL, and prompts from
matched Label Prompt Rules.

## Closed Work Item

A Work Item whose GitHub Issue is closed. Closed Work Items remain renderable
in issue views and keep their Session Attachment visible when one exists.
Closing the GitHub Issue does not detach the Session.

## Global GitHub Credential

The single AoE-wide credential used for GitHub issue workflows in v1. It is not
scoped per project, profile, host, or GitHub account.

## Issue Sync State

The per-project freshness and failure state for the local Issue Record cache.
It tells surfaces whether cached Work Items are current, stale, or blocked by
authentication or a GitHub failure.

## Issue-Backed Project

A saved AoE Project entry whose repository is eligible for GitHub issue sync.
Issue workflows require the repo to be saved as a Project entry in v1.
