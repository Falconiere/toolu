/** JiraTracker end to end through the real toolu jira CLI bundle (EPIC_JIRA_SH)
 * against the loopback HTTPS fixture posing as a Jira site. The bundle is a
 * Bun program published as jira.sh, so the tracker must exec it, not feed it
 * to bash. The tracker runs in its own process: the CLI it spawns inherits that
 * process's environment, which carries the fixture's proxy and CA. */

import { afterAll, beforeEach, expect, test } from "bun:test";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BUNDLE = join(import.meta.dir, "../../../jira/hooks/dist/jira.js");
const TRACKER = join(import.meta.dir, "../trackers/jira.ts");
const fixture = await startHttpsFixture(["acme.atlassian.net"]);
afterAll(() => fixture.stop());
beforeEach(() => fixture.plan([]));

/** Runs `new JiraTracker(ref).epic()` in a fresh bun process and returns its JSON. */
async function epicVia(ref: string): Promise<{ status: number; stdout: string; stderr: string }> {
  const dir = mkdtempSync(join(tmpdir(), "jira-tracker-"));
  try {
    const script = join(dir, "main.ts");
    writeFileSync(
      script,
      `import { JiraTracker } from ${JSON.stringify(TRACKER)};\n` +
        `console.log(JSON.stringify(await new JiraTracker(${JSON.stringify(ref)}, "acme/payments").epic()));\n`,
    );
    const child = Bun.spawn([process.execPath, script], {
      env: {
        ...process.env,
        ...fixture.env,
        EPIC_JIRA_SH: BUNDLE,
        JIRA_BASE_URL: "https://acme.atlassian.net",
        JIRA_PAT: "tok",
        JIRA_API_VERSION: "3",
        JIRA_CLI_CONFIG: "/dev/null",
        NETRC: "/dev/null",
      },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [stdout, stderr, status] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { status, stdout, stderr };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("epic() reads the epic through the jira CLI bundle", async () => {
  fixture.plan([
    {
      body: JSON.stringify({
        key: "PAY-7",
        fields: { summary: "Refunds epic", status: { statusCategory: { key: "done" } } },
      }),
    },
  ]);
  const run = await epicVia("PAY-7");
  expect(run.stderr).toBe("");
  expect(run.status).toBe(0);
  expect(JSON.parse(run.stdout)).toEqual({
    ref: "PAY-7",
    title: "Refunds epic",
    state: "closed",
    url: "https://acme.atlassian.net/browse/PAY-7",
  });
  expect(fixture.requests).toHaveLength(1);
  expect(fixture.requests[0]).toMatchObject({
    method: "GET",
    path: "/rest/api/3/issue/PAY-7?fields=summary,status",
  });
  expect(fixture.requests[0]?.headers["authorization"]).toBe("Bearer tok");
});
