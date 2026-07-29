import { ExternalLink, GitPullRequest, Tag } from "lucide-react";
import type { WorkItemProjection } from "../lib/types";

interface Props {
  item: WorkItemProjection | null;
  onCreateSession?: (item: WorkItemProjection) => void;
  readOnly?: boolean;
}

export function IssueDetailsPane({ item, onCreateSession, readOnly }: Props) {
  if (!item) {
    return (
      <div className="flex h-full min-h-0 flex-col items-center justify-center bg-surface-900 px-4 text-center">
        <p className="text-sm text-text-muted">Select an issue to view details.</p>
      </div>
    );
  }

  const body = item.issue.body?.trim() || item.issue.excerpt?.trim() || "";
  const issueParts = item.issue_ref.split("#");
  const issueNumber = issueParts[issueParts.length - 1];

  return (
    <div className="flex h-full min-h-0 flex-col bg-surface-900">
      <div className="border-b border-surface-700/60 px-4 py-3">
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <div className="mb-1 flex flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] text-text-dim">{item.issue_ref}</span>
              <span
                className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
                  item.state === "open" ? "bg-emerald-500/10 text-emerald-300" : "bg-surface-700/50 text-text-muted"
                }`}
              >
                {item.state}
              </span>
              {item.pull_request && (
                <span className="inline-flex items-center gap-1 rounded-full border border-violet-700/40 bg-violet-950/30 px-2 py-0.5 text-[11px] font-medium text-violet-300">
                  <GitPullRequest className="h-3 w-3" />
                  PR
                </span>
              )}
            </div>
            <h2 className="text-base font-semibold leading-snug text-text-primary">{item.title}</h2>
          </div>
          <a
            href={item.url}
            target="_blank"
            rel="noreferrer"
            aria-label={`Open issue ${issueNumber ?? item.issue_ref} on GitHub`}
            title="Open on GitHub"
            className="mt-0.5 inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-text-muted hover:bg-surface-800 hover:text-text-primary"
          >
            <ExternalLink className="h-4 w-4" />
          </a>
        </div>

        {item.labels.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {item.labels.map((label) => (
              <span
                key={label.name}
                title={label.description ?? label.name}
                className="inline-flex max-w-full items-center gap-1 rounded border border-surface-700/50 bg-surface-800/50 px-1.5 py-0.5 text-[11px] text-text-secondary"
              >
                <Tag className="h-3 w-3 shrink-0" />
                <span className="truncate">{label.name}</span>
              </span>
            ))}
          </div>
        )}

        {!readOnly && !item.attached_session_id && onCreateSession && (
          <button
            type="button"
            onClick={() => onCreateSession(item)}
            className="mt-3 inline-flex items-center rounded-md bg-brand-600 px-3 py-1.5 text-[13px] font-medium text-white hover:bg-brand-500"
          >
            New session
          </button>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {body ? (
          <div className="whitespace-pre-wrap text-sm leading-6 text-text-secondary">{body}</div>
        ) : (
          <p className="text-sm text-text-dim">No issue body cached.</p>
        )}
      </div>
    </div>
  );
}
