import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createGitHubIssue, editGitHubIssue, setGitHubIssueState } from "../api";

const fetchSpy = vi.fn<typeof fetch>();

beforeEach(() => {
  fetchSpy.mockReset();
  vi.stubGlobal("fetch", fetchSpy);
});

afterEach(() => vi.unstubAllGlobals());

describe("GitHub issue API wiring", () => {
  it("creates an issue through the dashboard endpoint", async () => {
    fetchSpy.mockResolvedValue(new Response(JSON.stringify({ issue: { issue_ref: "acme/app#12" } }), { status: 201 }));

    await createGitHubIssue({ owner: "acme", repo: "app", title: "Fix it", body: "Details" });

    expect(fetchSpy).toHaveBeenCalledWith(
      "/api/github/issues",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ owner: "acme", repo: "app", title: "Fix it", body: "Details" }),
      }),
    );
  });

  it("uses the shared issue mutation routes for edit and state changes", async () => {
    fetchSpy.mockResolvedValue(new Response(JSON.stringify({ issue: {} }), { status: 200 }));

    await editGitHubIssue("acme/app#12", { title: "Updated", labels: ["bug"] });
    await setGitHubIssueState("acme/app#12", "closed");

    expect(fetchSpy.mock.calls[0][0]).toBe("/api/github/issues/acme/app/12");
    expect(fetchSpy.mock.calls[0][1]).toMatchObject({ method: "PATCH" });
    expect(fetchSpy.mock.calls[1][0]).toBe("/api/github/issues/acme/app/12/state");
    expect(fetchSpy.mock.calls[1][1]).toMatchObject({ method: "PUT" });
  });
});
