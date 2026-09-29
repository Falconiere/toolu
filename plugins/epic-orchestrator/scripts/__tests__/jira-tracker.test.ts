/** JiraTracker end to end through the real toolu jira CLI bundle (EPIC_JIRA_SH)
 * against the loopback HTTPS fixture posing as a Jira site. The bundle is a
 * Bun program published as jira.sh, so the tracker must exec it, not feed it
 * to bash. The tracker runs in its own process: the CLI it spawns inherits that
 * process's environment, which carries the fixture's proxy and CA. */

import { expect, test } from "bun:test";
import { startHttpsFixture, type HttpsFixture } from "@toolu/conformance/https-fixture";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { join } from "node:path";

const BUNDLE = join(import.meta.dir, "../../../jira/hooks/dist/jira.js");
const TRACKER = join(import.meta.dir, "../trackers/jira.ts");

/** Runs `new JiraTracker(ref).epic()` in a fresh bun process and returns its result. */
function epicVia(sb: Sandbox, fixture: HttpsFixture, ref: string): Promise<RunResult> {
  const script = sb.write(
    "main.ts",
    `import { JiraTracker } from ${JSON.stringify(TRACKER)};\n` +
      `console.log(JSON.stringify(await new JiraTracker(${JSON.stringify(ref)}, "acme/payments").epic()));\n`,
  );
  return run([process.execPath, script], {
    env: {
      ...fixture.env,
      EPIC_JIRA_SH: BUNDLE,
      JIRA_BASE_URL: "https://acme.atlassian.net",
      JIRA_PAT: "tok",
      JIRA_API_VERSION: "3",
      JIRA_CLI_CONFIG: "/dev/null",
      NETRC: "/dev/null",
    },
  });
}

test.concurrent("epic() reads the epic through the jira CLI bundle", async () => {
  using sb = createSandbox();
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  try {
    fixture.plan([
      {
        body: JSON.stringify({
          key: "PAY-7",
          fields: { summary: "Refunds epic", status: { statusCategory: { key: "done" } } },
        }),
      },
    ]);
    const res = await epicVia(sb, fixture, "PAY-7");
    expect(res.stderr).toBe("");
    expect(res.exitCode).toBe(0);
    expect(JSON.parse(res.stdout)).toEqual({
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
  } finally {
    await fixture.stop();
  }
});
