import { useState } from "react";
import { ExternalLink, GitPullRequest, Pencil, Tag } from "lucide-react";
import type { WorkItemProjection } from "../lib/types";
import { issueLabelStyle } from "../lib/issueLabelColor";
import { editGitHubIssue, setGitHubIssueState } from "../lib/api";

interface Props {
  item: WorkItemProjection | null;
  onCreateSession?: (item: WorkItemProjection) => void;
  onDetachSession?: (sessionId: string) => Promise<boolean>;
  readOnly?: boolean;
}

export function IssueDetailsPane({ item, onCreateSession, onDetachSession, readOnly }: Props) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [labels, setLabels] = useState("");
  const [busy, setBusy] = useState(false);
  const [localItem, setLocalItem] = useState<WorkItemProjection | null>(null);
  const displayItem = localItem?.issue_ref === item?.issue_ref ? localItem : item;

  if (!displayItem) {
    return (
      <div className="flex h-full min-h-0 items-center justify-center bg-surface-900 px-4 text-center text-sm text-text-muted">
        Select an issue to view details.
      </div>
    );
  }

  const renderedBody = displayItem.issue.body?.trim() || displayItem.issue.excerpt?.trim() || "";
  const issueNumber = displayItem.issue_ref.split("#").at(-1);
  const beginEdit = () => {
    setTitle(displayItem.title);
    setBody(displayItem.issue.body ?? "");
    setLabels(displayItem.labels.map((label) => label.name).join(", "));
    setEditing(true);
  };

  return (
    <div className="flex h-full min-h-0 flex-col bg-surface-900">
      <div className="border-b border-surface-700/60 px-4 py-3">
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <div className="mb-1 flex flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] text-text-dim">{displayItem.issue_ref}</span>
              <span
                className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${displayItem.state === "open" ? "bg-emerald-500/10 text-emerald-300" : "bg-surface-700/50 text-text-muted"}`}
              >
                {displayItem.state}
              </span>
              {displayItem.pull_request && (
                <span className="inline-flex items-center gap-1 rounded-full border border-violet-700/40 bg-violet-950/30 px-2 py-0.5 text-[11px] text-violet-300">
                  <GitPullRequest className="h-3 w-3" />
                  PR
                </span>
              )}
            </div>
            <h2 className="text-base font-semibold leading-snug text-text-primary">{displayItem.title}</h2>
          </div>
          <a
            href={displayItem.url}
            target="_blank"
            rel="noreferrer"
            aria-label={`Open issue ${issueNumber ?? displayItem.issue_ref} on GitHub`}
            title="Open on GitHub"
            className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-text-muted hover:bg-surface-800 hover:text-text-primary"
          >
            <ExternalLink className="h-4 w-4" />
          </a>
          {!readOnly && (
            <button
              type="button"
              onClick={beginEdit}
              aria-label="Edit issue"
              title="Edit issue"
              className="inline-flex h-8 w-8 items-center justify-center rounded-md text-text-muted hover:bg-surface-800 hover:text-text-primary"
            >
              <Pencil className="h-4 w-4" />
            </button>
          )}
        </div>

        {editing && (
          <div className="mt-3 space-y-2 rounded-md border border-surface-700/50 bg-surface-800/40 p-3">
            <input
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              aria-label="Issue title"
              className="w-full rounded border border-surface-700 bg-surface-900 px-2 py-1.5 text-sm text-text-primary"
            />
            <textarea
              value={body}
              onChange={(event) => setBody(event.target.value)}
              aria-label="Issue body"
              rows={5}
              className="w-full rounded border border-surface-700 bg-surface-900 px-2 py-1.5 text-sm text-text-primary"
            />
            <input
              value={labels}
              onChange={(event) => setLabels(event.target.value)}
              aria-label="Issue labels"
              placeholder="labels, comma separated"
              className="w-full rounded border border-surface-700 bg-surface-900 px-2 py-1.5 text-sm text-text-primary"
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setEditing(false)}
                className="rounded px-2 py-1 text-xs text-text-muted"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  const result = await editGitHubIssue(displayItem.issue_ref, {
                    title,
                    body,
                    labels: labels
                      .split(",")
                      .map((label) => label.trim())
                      .filter(Boolean),
                  });
                  setBusy(false);
                  if (result.ok && result.issue) {
                    setLocalItem({
                      ...displayItem,
                      title: result.issue.title,
                      labels: result.issue.labels,
                      issue: result.issue,
                    });
                    setEditing(false);
                  }
                }}
                className="rounded bg-brand-600 px-2 py-1 text-xs text-white disabled:opacity-50"
              >
                {busy ? "Saving..." : "Save"}
              </button>
            </div>
          </div>
        )}

        {displayItem.labels.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {displayItem.labels.map((label) => (
              <span
                key={label.name}
                style={issueLabelStyle(label.color)}
                title={label.description ?? label.name}
                className="inline-flex max-w-full items-center gap-1 rounded border border-surface-700/50 bg-surface-800/50 px-1.5 py-0.5 text-[11px] text-text-secondary"
              >
                <Tag className="h-3 w-3 shrink-0" />
                <span className="truncate">{label.name}</span>
              </span>
            ))}
          </div>
        )}

        {!readOnly && !displayItem.attached_session_id && onCreateSession && (
          <button
            type="button"
            onClick={() => onCreateSession(displayItem)}
            className="mt-3 inline-flex items-center rounded-md bg-brand-600 px-3 py-1.5 text-[13px] font-medium text-white hover:bg-brand-500"
          >
            New session
          </button>
        )}
        {!readOnly && displayItem.attached_session_id && onDetachSession && (
          <button
            type="button"
            disabled={busy}
            onClick={() => onDetachSession(displayItem.attached_session_id!)}
            className="mt-3 ml-2 inline-flex items-center rounded-md border border-surface-700 px-3 py-1.5 text-[13px] text-text-secondary hover:bg-surface-800 disabled:opacity-50"
          >
            Detach session
          </button>
        )}
        {!readOnly && (
          <button
            type="button"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              const result = await setGitHubIssueState(
                displayItem.issue_ref,
                displayItem.state === "open" ? "closed" : "open",
              );
              setBusy(false);
              if (result.ok && result.issue)
                setLocalItem({
                  ...displayItem,
                  title: result.issue.title,
                  state: result.issue.state,
                  labels: result.issue.labels,
                  issue: result.issue,
                });
            }}
            className="mt-3 ml-2 inline-flex items-center rounded-md border border-surface-700 px-3 py-1.5 text-[13px] text-text-secondary hover:bg-surface-800 disabled:opacity-50"
          >
            {displayItem.state === "open" ? "Close issue" : "Reopen issue"}
          </button>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {renderedBody ? (
          <div className="whitespace-pre-wrap text-sm leading-6 text-text-secondary">{renderedBody}</div>
        ) : (
          <p className="text-sm text-text-dim">No issue body cached.</p>
        )}
      </div>
    </div>
  );
}
