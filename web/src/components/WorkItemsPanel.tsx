import { useEffect, useMemo, useState } from "react";
import { fetchWorkItems } from "../lib/api";
import type { AttentionState, ProjectInfo, SessionResponse, WorkItemProjection } from "../lib/types";

interface Props {
  projects: ProjectInfo[];
  sessions: SessionResponse[];
  readOnly?: boolean;
  onCreateFromIssue: (project: ProjectInfo, item: WorkItemProjection) => void;
  onSelectSession: (sessionId: string) => void;
  onAttachIssue: (sessionId: string, issueRef: string) => Promise<boolean>;
  onDetachIssue: (sessionId: string) => Promise<boolean>;
}

export function WorkItemsPanel({
  projects,
  sessions,
  readOnly,
  onCreateFromIssue,
  onSelectSession,
  onAttachIssue,
  onDetachIssue,
}: Props) {
  const githubProjects = useMemo(() => projects.filter((p) => p.github_repository), [projects]);
  const [projectPath, setProjectPath] = useState("");
  const [snapshot, setSnapshot] = useState<{
    slug: string;
    items: WorkItemProjection[];
    closedCount: number;
  } | null>(null);
  const [busyIssueRef, setBusyIssueRef] = useState<string | null>(null);
  const [attachTargets, setAttachTargets] = useState<Record<string, string>>({});

  const selectedProject = githubProjects.find((p) => p.path === projectPath) ?? githubProjects[0] ?? null;

  useEffect(() => {
    let active = true;
    const slug = selectedProject?.github_repository;
    if (!slug) return;
    const [owner, repo] = slug.split("/");
    if (!owner || !repo) return;
    void fetchWorkItems(owner, repo).then((res) => {
      if (!active) return;
      setSnapshot({
        slug,
        items: res?.work_items.open ?? [],
        closedCount: res?.work_items.closed.length ?? 0,
      });
    });
    return () => {
      active = false;
    };
  }, [selectedProject?.github_repository]);

  const sessionsByIssue = useMemo(() => {
    const map = new Map<string, SessionResponse>();
    for (const session of sessions) {
      if (session.issue_ref) map.set(session.issue_ref, session);
    }
    return map;
  }, [sessions]);

  const attachableSessions = sessions.filter(
    (s) =>
      !s.trashed_at && !s.issue_ref && selectedProject && (s.main_repo_path || s.project_path) === selectedProject.path,
  );
  if (githubProjects.length === 0) return null;
  const selectedSlug = selectedProject?.github_repository ?? "";
  const loading = !!selectedSlug && snapshot?.slug !== selectedSlug;
  const items = snapshot?.slug === selectedSlug ? snapshot.items : [];
  const closedCount = snapshot?.slug === selectedSlug ? snapshot.closedCount : 0;

  return (
    <section className="w-full max-w-2xl mt-4 border-t border-surface-800 pt-4" aria-label="Work Items">
      <div className="flex items-center gap-2 mb-2">
        <h2 className="text-sm font-medium text-text-secondary flex-1">Work Items</h2>
        {githubProjects.length > 1 && (
          <select
            value={selectedProject?.path ?? ""}
            onChange={(e) => setProjectPath(e.target.value)}
            className="bg-surface-900 border border-surface-700 rounded px-2 py-1 text-xs text-text-secondary"
            aria-label="Work item project"
          >
            {githubProjects.map((p) => (
              <option key={`${p.scope}:${p.path}`} value={p.path}>
                {p.github_repository}
              </option>
            ))}
          </select>
        )}
      </div>
      <div className="border border-surface-800 rounded-lg overflow-hidden bg-surface-900/60">
        {loading ? (
          <div className="px-3 py-3 text-sm text-text-dim">Loading issues...</div>
        ) : items.length === 0 ? (
          <div className="px-3 py-3 text-sm text-text-dim">
            {closedCount > 0 ? `${closedCount} closed issue${closedCount === 1 ? "" : "s"}` : "No cached open issues"}
          </div>
        ) : (
          items.map((item) => {
            const attached = sessionsByIssue.get(item.issue_ref);
            const attachTarget = attachTargets[item.issue_ref] ?? attachableSessions[0]?.id ?? "";
            const busy = busyIssueRef === item.issue_ref;
            return (
              <div
                key={item.issue_ref}
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "n" && !attached && !readOnly && selectedProject)
                    onCreateFromIssue(selectedProject, item);
                }}
                className="px-3 py-2 border-b border-surface-800 last:border-b-0 focus:outline-none focus:bg-surface-850"
              >
                <div className="flex items-start gap-3">
                  <div className="min-w-0 flex-1">
                    <div className="text-sm text-text-primary truncate">{item.title}</div>
                    <div className="text-xs font-mono text-text-dim">{item.issue_ref}</div>
                  </div>
                  {item.attached_session_id && item.attention_state && (
                    <span
                      className={`mt-1 h-2.5 w-2.5 shrink-0 rounded-full ${attentionDotClass(item.attention_state)}`}
                      title={attentionLabel(item.attention_state)}
                      aria-label={attentionLabel(item.attention_state)}
                      data-attention-state={item.attention_state}
                    />
                  )}
                  {attached ? (
                    <div className="flex items-center gap-2 shrink-0">
                      <button
                        className="px-2 py-1 rounded bg-surface-800 text-xs text-text-secondary hover:bg-surface-700"
                        onClick={() => onSelectSession(attached.id)}
                      >
                        Open
                      </button>
                      {!readOnly && (
                        <button
                          className="px-2 py-1 rounded bg-surface-800 text-xs text-text-secondary hover:bg-surface-700 disabled:opacity-50"
                          disabled={busy}
                          onClick={async () => {
                            setBusyIssueRef(item.issue_ref);
                            await onDetachIssue(attached.id);
                            setBusyIssueRef(null);
                          }}
                        >
                          Detach
                        </button>
                      )}
                    </div>
                  ) : !readOnly ? (
                    <div className="flex items-center gap-2 shrink-0">
                      {attachableSessions.length > 0 && (
                        <>
                          <select
                            value={attachTarget}
                            onChange={(e) =>
                              setAttachTargets((prev) => ({ ...prev, [item.issue_ref]: e.target.value }))
                            }
                            className="max-w-32 bg-surface-950 border border-surface-700 rounded px-2 py-1 text-xs text-text-secondary"
                            aria-label={`Attach session to ${item.issue_ref}`}
                          >
                            {attachableSessions.map((s) => (
                              <option key={s.id} value={s.id}>
                                {s.title}
                              </option>
                            ))}
                          </select>
                          <button
                            className="px-2 py-1 rounded bg-surface-800 text-xs text-text-secondary hover:bg-surface-700 disabled:opacity-50"
                            disabled={!attachTarget || busy}
                            onClick={async () => {
                              if (!attachTarget) return;
                              setBusyIssueRef(item.issue_ref);
                              await onAttachIssue(attachTarget, item.issue_ref);
                              setBusyIssueRef(null);
                            }}
                          >
                            Attach
                          </button>
                        </>
                      )}
                      <button
                        className="px-2 py-1 rounded bg-brand-600 text-xs text-white hover:bg-brand-500"
                        onClick={() => selectedProject && onCreateFromIssue(selectedProject, item)}
                      >
                        New
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}

function attentionDotClass(state: AttentionState): string {
  switch (state) {
    case "needs_input":
      return "bg-amber-400";
    case "error":
      return "bg-red-500";
    case "idle":
      return "bg-surface-500";
    case "active":
      return "bg-emerald-500";
    case "stopped":
      return "bg-surface-600";
  }
}

function attentionLabel(state: AttentionState): string {
  switch (state) {
    case "needs_input":
      return "Needs input";
    case "error":
      return "Error";
    case "idle":
      return "Idle";
    case "active":
      return "Active";
    case "stopped":
      return "Stopped";
  }
}
