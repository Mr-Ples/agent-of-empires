// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import { WorkItemsSidebar } from "../WorkItemsSidebar";
import { fetchWorkItems } from "../../lib/api";
import type { ProjectInfo, SessionResponse, WorkItemProjection, WorkItemsResponse } from "../../lib/types";

vi.mock("../../lib/api", () => ({
  fetchWorkItems: vi.fn(),
}));

const project: ProjectInfo = {
  name: "AoE",
  path: "/repo",
  scope: "global",
  pinned: true,
  github_repository: "mr-ples/agent-of-empires",
};

const otherProject: ProjectInfo = {
  name: "Other",
  path: "/other",
  scope: "global",
  pinned: true,
  github_repository: "mr-ples/other",
};

function session(over: Partial<SessionResponse>): SessionResponse {
  return {
    id: "s1",
    title: "Attached session",
    project_path: "/repo",
    main_repo_path: "/repo",
    status: "Idle",
    issue_ref: null,
    trashed_at: null,
    ...over,
  } as SessionResponse;
}

function item(over: Partial<WorkItemProjection> = {}): WorkItemProjection {
  const issue = {
    issue_ref: over.issue_ref ?? "mr-ples/agent-of-empires#22",
    github_id: 22,
    node_id: "node",
    title: over.title ?? "Build web Issues sidebar read path",
    body: "Issue body",
    excerpt: null,
    state: over.state ?? "open",
    labels: [{ name: "ready-for-agent", color: "0e8a16", description: null }],
    assignees: [],
    url: "https://github.com/Mr-Ples/agent-of-empires/issues/22",
    created_at: "2026-07-29T00:00:00Z",
    updated_at: "2026-07-29T00:00:00Z",
    closed_at: null,
    pull_request: null,
    sync: { status: "fresh", synced_at: "2026-07-29T00:00:00Z", message: null },
  } as WorkItemProjection["issue"];
  return {
    issue_ref: issue.issue_ref,
    title: issue.title,
    state: issue.state,
    attached_session_id: null,
    runtime_liveness: null,
    attention_state: null,
    labels: issue.labels,
    url: issue.url,
    pull_request: null,
    sync: issue.sync,
    issue,
    ...over,
  };
}

function response(open: WorkItemProjection[], closed: WorkItemProjection[] = []): WorkItemsResponse {
  return {
    repository: { owner: "mr-ples", repo: "agent-of-empires" },
    sync: { status: "fresh", synced_at: "2026-07-29T00:00:00Z", message: null },
    work_items: { open, closed },
  };
}

afterEach(() => cleanup());

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(fetchWorkItems).mockResolvedValue(response([item()]));
});

describe("WorkItemsSidebar", () => {
  it("loads project-scoped issues and selects unattached rows directly", async () => {
    const onSelectIssue = vi.fn();
    render(
      <WorkItemsSidebar
        projects={[project]}
        sessions={[]}
        activeProjectPath={null}
        selectedIssueRef={null}
        onSelectIssue={onSelectIssue}
        onCreateFromIssue={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByText("Build web Issues sidebar read path"));

    expect(fetchWorkItems).toHaveBeenCalledWith("mr-ples", "agent-of-empires");
    expect(onSelectIssue).toHaveBeenCalledWith(
      project,
      expect.objectContaining({ issue_ref: project.github_repository + "#22" }),
    );
  });

  it("keeps attached rows session-first and shows the attention dot only for attachments", async () => {
    vi.mocked(fetchWorkItems).mockResolvedValue(
      response([
        item({ attached_session_id: "s1", attention_state: "needs_input", runtime_liveness: "idle" }),
        item({ issue_ref: "mr-ples/agent-of-empires#23", title: "Unattached follow-up" }),
      ]),
    );
    const onSelectIssue = vi.fn();

    render(
      <WorkItemsSidebar
        projects={[project]}
        sessions={[session({ id: "s1", issue_ref: "mr-ples/agent-of-empires#22" })]}
        activeProjectPath={null}
        selectedIssueRef="mr-ples/agent-of-empires#22"
        onSelectIssue={onSelectIssue}
        onCreateFromIssue={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByText("Build web Issues sidebar read path"));
    expect(screen.getByLabelText("Needs input").getAttribute("data-attention-state")).toBe("needs_input");
    const attachedRow = screen
      .getByText("Build web Issues sidebar read path")
      .closest("[data-testid='sidebar-issue-row']");
    const unattachedRow = screen.getByText("Unattached follow-up").closest("[data-testid='sidebar-issue-row']");
    expect(attachedRow?.getAttribute("data-attached")).toBe("true");
    expect(unattachedRow?.getAttribute("data-attached")).toBe("false");
    expect(onSelectIssue).toHaveBeenCalledWith(project, expect.objectContaining({ attached_session_id: "s1" }));
  });

  it("supports n on unattached issue rows and hides closed issues until expanded", async () => {
    const onCreateFromIssue = vi.fn();
    vi.mocked(fetchWorkItems).mockResolvedValue(
      response([item()], [item({ issue_ref: "mr-ples/agent-of-empires#9", title: "Closed bug", state: "closed" })]),
    );

    render(
      <WorkItemsSidebar
        projects={[project]}
        sessions={[]}
        activeProjectPath={null}
        selectedIssueRef={null}
        onSelectIssue={vi.fn()}
        onCreateFromIssue={onCreateFromIssue}
      />,
    );

    const row = (await screen.findByText("Build web Issues sidebar read path")).closest(
      "[data-testid='sidebar-issue-row']",
    )!;
    fireEvent.keyDown(row, { key: "n" });
    expect(onCreateFromIssue).toHaveBeenCalledWith(
      project,
      expect.objectContaining({ issue_ref: "mr-ples/agent-of-empires#22" }),
    );

    expect(screen.queryByText("Closed bug")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Closed issues (1)" }));
    await waitFor(() => expect(screen.getByText("Closed bug")).toBeTruthy());
  });

  it("defaults the issue project to the active session project", async () => {
    render(
      <WorkItemsSidebar
        projects={[otherProject, project]}
        sessions={[]}
        activeProjectPath="/repo"
        selectedIssueRef={null}
        onSelectIssue={vi.fn()}
        onCreateFromIssue={vi.fn()}
      />,
    );

    await screen.findByText("Build web Issues sidebar read path");
    expect(fetchWorkItems).toHaveBeenCalledWith("mr-ples", "agent-of-empires");
  });
});
