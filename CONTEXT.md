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
