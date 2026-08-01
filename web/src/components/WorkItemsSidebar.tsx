import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { GitPullRequest, RefreshCw } from "lucide-react";
import { fetchWorkItems } from "../lib/api";
import type {
  AttentionState,
  IssueSyncMetadata,
  ProjectInfo,
  SessionResponse,
  WorkItemProjection,
} from "../lib/types";
import { issueLabelStyle } from "../lib/issueLabelColor";
import { Tooltip } from "./Tooltip";

interface Props {
  projects: ProjectInfo[];
  sessions: SessionResponse[];
  activeProjectPath: string | null;
  selectedIssueRef: string | null;
  readOnly?: boolean;
  onSelectIssue: (project: ProjectInfo, item: WorkItemProjection) => void;
  onCreateFromIssue: (project: ProjectInfo, item: WorkItemProjection) => void;
}

export function WorkItemsSidebar({
  projects,
  sessions,
  activeProjectPath,
  selectedIssueRef,
  readOnly,
  onSelectIssue,
  onCreateFromIssue,
}: Props) {
  const githubProjects = useMemo(() => projects.filter((p) => p.github_repository), [projects]);
  const [projectPath, setProjectPath] = useState("");
  const [snapshot, setSnapshot] = useState<{
    slug: string;
    open: WorkItemProjection[];
    closed: WorkItemProjection[];
    sync: IssueSyncMetadata | null;
  } | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [closedExpanded, setClosedExpanded] = useState(false);
  const requestVersion = useRef(0);
  const selectedProject =
    githubProjects.find((p) => p.path === projectPath) ??
    githubProjects.find((p) => p.path === activeProjectPath) ??
    githubProjects[0] ??
    null;
  const sessionsByIssue = useMemo(() => {
    const map = new Map<string, SessionResponse>();
    for (const session of sessions) {
      if (session.issue_ref) map.set(session.issue_ref, session);
    }
    return map;
  }, [sessions]);

  const load = useCallback(() => {
    const slug = selectedProject?.github_repository;
    if (!slug) return;
    const [owner, repo] = slug.split("/");
    if (!owner || !repo) return;
    const version = ++requestVersion.current;
    void fetchWorkItems(owner, repo).then((res) => {
      if (version !== requestVersion.current) return;
      if (!res) {
        setLoadError(true);
        return;
      }
      setLoadError(false);
      setSnapshot({
        slug,
        open: res.work_items.open,
        closed: res.work_items.closed,
        sync: res.sync,
      });
    });
  }, [selectedProject?.github_repository]);

  useEffect(() => {
    setLoadError(false);
    load();
    const refreshTimer = window.setInterval(load, 10_000);
    return () => window.clearInterval(refreshTimer);
  }, [load]);

  if (githubProjects.length === 0) {
    return (
      <div className="flex h-full flex-col">
        <IssueModeHeader />
        <div className="flex-1 border-t border-surface-700/60 px-4 py-10 text-center">
          <p className="text-sm font-medium text-text-secondary">No GitHub projects</p>
          <p className="mt-1 text-[13px] text-text-muted">Save a project with a GitHub remote to show issues.</p>
        </div>
      </div>
    );
  }

  const selectedSlug = selectedProject?.github_repository ?? "";
  const items = snapshot?.slug === selectedSlug ? snapshot.open : [];
  const closed = snapshot?.slug === selectedSlug ? snapshot.closed : [];
  const sync = snapshot?.slug === selectedSlug ? snapshot.sync : null;
  const isLoading = !!selectedSlug && snapshot?.slug !== selectedSlug && !loadError;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <IssueModeHeader onRefresh={load} loading={isLoading} sync={sync} />
      {githubProjects.length > 1 && (
        <div className="px-3 pb-2">
          <select
            value={selectedProject?.path ?? ""}
            onChange={(e) => setProjectPath(e.target.value)}
            className="w-full rounded-md border border-surface-700 bg-surface-900 px-2 py-1.5 text-xs text-text-secondary"
            aria-label="Issue project"
          >
            {githubProjects.map((project) => (
              <option key={`${project.scope}:${project.path}`} value={project.path}>
                {project.github_repository}
              </option>
            ))}
          </select>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden border-t border-surface-700/60">
        {isLoading ? (
          <div className="px-4 py-8 text-center text-sm text-text-dim">Loading issues...</div>
        ) : loadError && items.length === 0 && closed.length === 0 ? (
          <div className="px-4 py-10 text-center">
            <p className="text-sm font-medium text-text-secondary">Issues unavailable</p>
            <p className="mt-1 text-[13px] text-text-muted">Refresh to retry loading the cached issue list.</p>
          </div>
        ) : items.length === 0 && closed.length === 0 ? (
          <div className="px-4 py-10 text-center">
            <p className="text-sm font-medium text-text-secondary">No cached issues</p>
            <p className="mt-1 text-[13px] text-text-muted">Sync this project to populate Work Items.</p>
          </div>
        ) : (
          <>
            {items.map((item) => {
              const attached = sessionsByIssue.get(item.issue_ref) ?? null;
              const effectiveItem = withEffectiveAttachment(item, attached);
              return (
                <IssueRow
                  key={item.issue_ref}
                  item={effectiveItem}
                  attached={attached}
                  selected={selectedIssueRef === item.issue_ref}
                  onSelect={() => selectedProject && onSelectIssue(selectedProject, effectiveItem)}
                  onCreate={() => selectedProject && onCreateFromIssue(selectedProject, effectiveItem)}
                  readOnly={readOnly}
                />
              );
            })}
            {closed.length > 0 && (
              <div>
                <button
                  type="button"
                  onClick={() => setClosedExpanded((v) => !v)}
                  className="flex w-full items-center gap-2 border-t border-surface-800/60 px-3 py-1.5 text-left font-mono text-[11px] uppercase tracking-widest text-text-muted hover:bg-surface-800/40 hover:text-text-secondary"
                  aria-expanded={closedExpanded}
                >
                  <span aria-hidden>{closedExpanded ? "⌄" : "›"}</span>
                  Closed issues ({closed.length})
                </button>
                {closedExpanded &&
                  closed.map((item) => {
                    const attached = sessionsByIssue.get(item.issue_ref) ?? null;
                    const effectiveItem = withEffectiveAttachment(item, attached);
                    return (
                      <IssueRow
                        key={item.issue_ref}
                        item={effectiveItem}
                        attached={attached}
                        selected={selectedIssueRef === item.issue_ref}
                        onSelect={() => selectedProject && onSelectIssue(selectedProject, effectiveItem)}
                        onCreate={() => selectedProject && onCreateFromIssue(selectedProject, effectiveItem)}
                        readOnly={readOnly}
                      />
                    );
                  })}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

function withEffectiveAttachment(item: WorkItemProjection, attached: SessionResponse | null): WorkItemProjection {
  if (!attached || item.attached_session_id) return item;
  return { ...item, attached_session_id: attached.id };
}

function IssueModeHeader({
  loading,
  onRefresh,
  sync,
}: {
  loading?: boolean;
  onRefresh?: () => void;
  sync?: IssueSyncMetadata | null;
}) {
  const syncLabel = sync ? issueSyncLabel(sync.status) : null;
  const syncNeedsAttention = sync && sync.status !== "fresh";

  return (
    <div className="px-3 pb-2 pt-3">
      <div className="flex items-center gap-1">
        <span className="flex-1 text-sm text-text-muted">Issues</span>
        {onRefresh && (
          <Tooltip text="Refresh issues">
            <button
              type="button"
              onClick={onRefresh}
              disabled={loading}
              aria-label="Refresh issues"
              className="flex h-8 w-8 items-center justify-center rounded-md text-text-dim hover:bg-surface-800 hover:text-text-secondary disabled:opacity-40"
            >
              <RefreshCw className={`h-3.5 w-3.5 ${loading ? "motion-safe:animate-spin" : ""}`} />
            </button>
          </Tooltip>
        )}
      </div>
      {syncLabel && (
        <p
          className={`mt-1 truncate text-[11px] ${syncNeedsAttention ? "text-amber-300" : "text-text-dim"}`}
          title={sync?.message ?? syncLabel}
          role={syncNeedsAttention ? "status" : undefined}
        >
          {syncLabel}
          {sync?.synced_at ? ` · ${formatSyncTime(sync.synced_at)}` : ""}
        </p>
      )}
    </div>
  );
}

function issueSyncLabel(status: IssueSyncMetadata["status"]): string {
  switch (status) {
    case "fresh":
      return "Synced";
    case "stale":
      return "Sync stale, showing cached issues";
    case "auth_required":
      return "GitHub authentication required";
    case "rate_limited":
      return "GitHub rate limited, showing cached issues";
    case "forbidden":
      return "GitHub access denied, showing cached issues";
    case "not_found":
      return "GitHub repository not found";
    case "network":
      return "GitHub unavailable, showing cached issues";
    case "api_failure":
      return "GitHub sync failed, showing cached issues";
  }
}

function formatSyncTime(value: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) return value;
  return new Date(timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function IssueRow({
  item,
  attached,
  selected,
  readOnly,
  onSelect,
  onCreate,
}: {
  item: WorkItemProjection;
  attached: SessionResponse | null;
  selected: boolean;
  readOnly?: boolean;
  onSelect: () => void;
  onCreate: () => void;
}) {
  const hasAttachedSession = !!attached || !!item.attached_session_id;
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter") onSelect();
        if (e.key === "n" && !hasAttachedSession && !readOnly) onCreate();
      }}
      data-testid="sidebar-issue-row"
      data-attached={hasAttachedSession ? "true" : "false"}
      className={`border-l-2 px-3 py-2 text-left hover:bg-surface-700/40 ${
        selected ? "border-brand-600 bg-surface-850" : "border-transparent"
      }`}
    >
      <div className="flex items-start gap-2">
        {hasAttachedSession && item.attention_state && (
          <span
            className={`mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full ${attentionDotClass(item.attention_state)}`}
            title={attentionLabel(item.attention_state)}
            aria-label={attentionLabel(item.attention_state)}
            data-attention-state={item.attention_state}
          />
        )}
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-1">
            <div
              className={`min-w-0 flex-1 truncate text-[13px] md:text-[14px] ${
                item.state === "closed" ? "text-text-muted line-through" : "text-text-secondary"
              }`}
              title={item.title}
            >
              {item.title}
            </div>
            <div className="ml-auto flex shrink-0 items-center gap-1">
              {item.labels.slice(0, 2).map((label) => (
                <span
                  key={label.name}
                  className="max-w-20 truncate rounded-full border border-surface-700/40 bg-surface-800/40 px-1 text-[9px] leading-4 text-text-dim"
                  style={issueLabelStyle(label.color)}
                  title={label.description ?? label.name}
                >
                  {label.name}
                </span>
              ))}
              {item.labels.length > 2 && (
                <span className="rounded-full border border-surface-700/40 bg-surface-800/40 px-1 text-[9px] leading-4 text-text-dim">
                  +{item.labels.length - 2}
                </span>
              )}
            </div>
          </div>
          <div className="mt-0.5 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            <span className="font-mono text-[10px] text-text-dim">{item.issue_ref}</span>
            {item.pull_request && (
              <span className="inline-flex items-center gap-0.5 rounded border border-violet-700/40 bg-violet-950/30 px-1 text-[10px] font-medium text-violet-300">
                <GitPullRequest className="h-3 w-3" />
                PR
              </span>
            )}
            {hasAttachedSession ? (
              <span className="rounded border border-brand-700/40 bg-brand-700/5 px-1 text-[10px] font-mono text-brand-300">
                attached
              </span>
            ) : null}
          </div>
        </div>
      </div>
    </div>
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
