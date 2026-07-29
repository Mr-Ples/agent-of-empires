// Live API coverage for the issue-first Work Item contract in #17.
//
// The test avoids live GitHub by seeding the normalized issue cache on disk,
// then drives the same backend endpoints the dashboard panel uses.

import { test, expect } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { appDirFor, listSessions, resolveAoeBinary, spawnAoeServe } from "../helpers/aoeServe";

const ISSUE_REF = "mr-ples/agent-of-empires#17";

function run(command: string, args: string[], options: { cwd?: string; env: NodeJS.ProcessEnv }) {
  const res = spawnSync(command, args, options);
  if (res.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed: status=${res.status} stderr=${res.stderr?.toString() ?? "<none>"}`,
    );
  }
}

function seedRepoSessionAndIssueCache({ home, xdg, env }: { home: string; xdg: string; env: NodeJS.ProcessEnv }) {
  const projectDir = join(home, "project");
  mkdirSync(projectDir, { recursive: true });
  const gitEnv = {
    ...env,
    GIT_AUTHOR_NAME: "t",
    GIT_AUTHOR_EMAIL: "t@t",
    GIT_COMMITTER_NAME: "t",
    GIT_COMMITTER_EMAIL: "t@t",
  };
  run("git", ["init", "-q"], { cwd: projectDir, env: gitEnv });
  run("git", ["remote", "add", "origin", "https://github.com/Mr-Ples/agent-of-empires.git"], {
    cwd: projectDir,
    env: gitEnv,
  });
  run("git", ["commit", "--allow-empty", "-q", "-m", "init"], { cwd: projectDir, env: gitEnv });
  run(resolveAoeBinary(), ["add", projectDir, "-t", "issue-existing", "-c", "claude"], { env });

  const appDir = appDirFor(home, xdg, resolveAoeBinary());
  const cacheDir = join(appDir, "github", "issues");
  mkdirSync(cacheDir, { recursive: true });
  writeFileSync(
    join(cacheDir, "mr-ples__agent-of-empires.json"),
    JSON.stringify(
      {
        repository: { owner: "mr-ples", repo: "agent-of-empires" },
        issues: [
          {
            issue_ref: ISSUE_REF,
            github_id: 17,
            node_id: "I_17",
            title: "Support issue-first session creation",
            body: "Acceptance criteria here.",
            excerpt: "Acceptance criteria here.",
            state: "open",
            labels: [{ name: "ready-for-agent", color: null, description: null }],
            assignees: [],
            url: "https://github.com/Mr-Ples/agent-of-empires/issues/17",
            created_at: "2026-07-29T00:00:00Z",
            updated_at: "2026-07-29T00:00:00Z",
            closed_at: null,
            pull_request: null,
            sync: { status: "fresh", synced_at: "2026-07-29T00:00:00Z", message: null },
          },
        ],
        sync: { status: "fresh", synced_at: "2026-07-29T00:00:00Z", message: null },
      },
      null,
      2,
    ),
  );
}

test("project slug, work item visibility, attach, and detach round trip", async ({ request }, testInfo) => {
  const serve = await spawnAoeServe({
    authMode: "none",
    workerIndex: testInfo.workerIndex,
    parallelIndex: testInfo.parallelIndex,
    seedFn: seedRepoSessionAndIssueCache,
  });

  try {
    const projectDir = join(serve.home, "project");
    const projectRes = await request.post(`${serve.baseUrl}/api/projects`, {
      data: { path: projectDir, scope: "global", pinned: true },
    });
    expect(projectRes.ok()).toBe(true);
    await expect(projectRes.json()).resolves.toMatchObject({
      path: projectDir,
      github_repository: "Mr-Ples/agent-of-empires",
    });

    const sessions = await listSessions(serve.baseUrl);
    const sessionId = sessions[0]!.id;

    const workItemsBefore = await (
      await request.get(`${serve.baseUrl}/api/work-items?owner=mr-ples&repo=agent-of-empires`)
    ).json();
    expect(workItemsBefore.work_items.open[0]).toMatchObject({
      issue_ref: ISSUE_REF,
      state: "open",
    });
    expect(workItemsBefore.work_items.open[0]).not.toHaveProperty("attached_session_id");

    const attachRes = await request.patch(`${serve.baseUrl}/api/sessions/${sessionId}/issue-ref`, {
      data: { issue_ref: ISSUE_REF },
    });
    expect(attachRes.ok()).toBe(true);
    await expect(attachRes.json()).resolves.toMatchObject({ id: sessionId, issue_ref: ISSUE_REF });

    const workItemsAttached = await (
      await request.get(`${serve.baseUrl}/api/work-items?owner=mr-ples&repo=agent-of-empires`)
    ).json();
    expect(workItemsAttached.work_items.open[0]).toMatchObject({
      issue_ref: ISSUE_REF,
      attached_session_id: sessionId,
      state: "open",
    });

    const detachRes = await request.patch(`${serve.baseUrl}/api/sessions/${sessionId}/issue-ref`, {
      data: { issue_ref: null },
    });
    expect(detachRes.ok()).toBe(true);
    const detachedSession = await detachRes.json();
    expect(detachedSession).toMatchObject({ id: sessionId });
    expect(detachedSession).not.toHaveProperty("issue_ref");

    const workItemsDetached = await (
      await request.get(`${serve.baseUrl}/api/work-items?owner=mr-ples&repo=agent-of-empires`)
    ).json();
    expect(workItemsDetached.work_items.open[0]).toMatchObject({
      issue_ref: ISSUE_REF,
      state: "open",
    });
    expect(workItemsDetached.work_items.open[0]).not.toHaveProperty("attached_session_id");
  } finally {
    await serve.stop();
  }
});
