// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import { WorkItemsPanel } from "../WorkItemsPanel";
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

function session(over: Partial<SessionResponse>): SessionResponse {
  return {
    id: "s1",
    title: "Existing session",
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
    issue_ref: "mr-ples/agent-of-empires#17",
    github_id: 17,
    node_id: "node",
    title: "Support issue-first session creation",
    body: "Body",
    excerpt: null,
    state: "open",
    labels: [],
    assignees: [],
    url: "https://github.com/Mr-Ples/agent-of-empires/issues/17",
    created_at: "2026-07-29T00:00:00Z",
    updated_at: "2026-07-29T00:00:00Z",
    closed_at: null,
    pull_request: null,
    sync: { status: "fresh", synced_at: "2026-07-29T00:00:00Z", message: null },
  } as WorkItemProjection["issue"];
  return {
    issue_ref: issue.issue_ref,
    title: issue.title,
    state: "open",
    attached_session_id: null,
    labels: [],
    url: issue.url,
    pull_request: null,
    sync: issue.sync,
    issue,
    ...over,
  };
}

function response(open: WorkItemProjection[]): WorkItemsResponse {
  return {
    repository: { owner: "mr-ples", repo: "agent-of-empires" },
    sync: { status: "fresh", synced_at: "2026-07-29T00:00:00Z", message: null },
    work_items: { open, closed: [] },
  };
}

afterEach(() => cleanup());

beforeEach(() => {
  vi.mocked(fetchWorkItems).mockResolvedValue(response([item()]));
});

describe("WorkItemsPanel", () => {
  it("loads cached work items for a saved GitHub project and creates from an unattached issue", async () => {
    const onCreateFromIssue = vi.fn();
    render(
      <WorkItemsPanel
        projects={[project]}
        sessions={[]}
        onCreateFromIssue={onCreateFromIssue}
        onSelectSession={vi.fn()}
        onAttachIssue={vi.fn()}
        onDetachIssue={vi.fn()}
      />,
    );

    await screen.findByText("Support issue-first session creation");
    expect(fetchWorkItems).toHaveBeenCalledWith("mr-ples", "agent-of-empires");

    fireEvent.click(screen.getByRole("button", { name: "New" }));
    expect(onCreateFromIssue).toHaveBeenCalledWith(
      project,
      expect.objectContaining({ issue_ref: "mr-ples/agent-of-empires#17" }),
    );
  });

  it("supports n from an unattached focused work item and disables it once attached", async () => {
    const onCreateFromIssue = vi.fn();
    const { rerender } = render(
      <WorkItemsPanel
        projects={[project]}
        sessions={[]}
        onCreateFromIssue={onCreateFromIssue}
        onSelectSession={vi.fn()}
        onAttachIssue={vi.fn()}
        onDetachIssue={vi.fn()}
      />,
    );

    const row = await screen.findByText("Support issue-first session creation");
    fireEvent.keyDown(row.closest("[tabindex]")!, { key: "n" });
    expect(onCreateFromIssue).toHaveBeenCalledTimes(1);

    rerender(
      <WorkItemsPanel
        projects={[project]}
        sessions={[session({ issue_ref: "mr-ples/agent-of-empires#17" })]}
        onCreateFromIssue={onCreateFromIssue}
        onSelectSession={vi.fn()}
        onAttachIssue={vi.fn()}
        onDetachIssue={vi.fn()}
      />,
    );
    fireEvent.keyDown(screen.getByText("Support issue-first session creation").closest("[tabindex]")!, { key: "n" });
    expect(onCreateFromIssue).toHaveBeenCalledTimes(1);
  });

  it("attaches existing sessions and detaches without hiding the work item", async () => {
    const onAttachIssue = vi.fn().mockResolvedValue(true);
    const onDetachIssue = vi.fn().mockResolvedValue(true);
    const { rerender } = render(
      <WorkItemsPanel
        projects={[project]}
        sessions={[session({ id: "s1", title: "Started work" })]}
        onCreateFromIssue={vi.fn()}
        onSelectSession={vi.fn()}
        onAttachIssue={onAttachIssue}
        onDetachIssue={onDetachIssue}
      />,
    );

    await screen.findByText("Support issue-first session creation");
    fireEvent.click(screen.getByRole("button", { name: "Attach" }));
    await waitFor(() => expect(onAttachIssue).toHaveBeenCalledWith("s1", "mr-ples/agent-of-empires#17"));

    rerender(
      <WorkItemsPanel
        projects={[project]}
        sessions={[session({ id: "s1", title: "Started work", issue_ref: "mr-ples/agent-of-empires#17" })]}
        onCreateFromIssue={vi.fn()}
        onSelectSession={vi.fn()}
        onAttachIssue={onAttachIssue}
        onDetachIssue={onDetachIssue}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Detach" }));
    await waitFor(() => expect(onDetachIssue).toHaveBeenCalledWith("s1"));
    expect(screen.getByText("Support issue-first session creation")).toBeTruthy();
  });

  it("does not offer sessions from another project as attach targets", async () => {
    render(
      <WorkItemsPanel
        projects={[project]}
        sessions={[
          session({ id: "other", title: "Other repo", project_path: "/other", main_repo_path: "/other" }),
          session({ id: "same", title: "Same repo", project_path: "/repo", main_repo_path: "/repo" }),
        ]}
        onCreateFromIssue={vi.fn()}
        onSelectSession={vi.fn()}
        onAttachIssue={vi.fn()}
        onDetachIssue={vi.fn()}
      />,
    );

    await screen.findByText("Support issue-first session creation");
    expect(screen.getByRole("option", { name: "Same repo" })).toBeTruthy();
    expect(screen.queryByRole("option", { name: "Other repo" })).toBeNull();
  });
});
