/** Jira tracker against the loopback HTTPS service, without the jira plugin. */
import { expect, test } from "bun:test";
import { startHttpsFixture, type HttpsFixture } from "@toolu/conformance/https-fixture";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";
import { join } from "node:path";

const TRACKER = join(import.meta.dir, "../trackers/jira.ts");
const BASE = "https://acme.atlassian.net";

function via(
  sb: Sandbox,
  fixture: HttpsFixture,
  method: string,
  args: unknown[] = [],
  env: EnvPatch = {},
): Promise<RunResult> {
  const script = sb.write(
    "main.ts",
    "import { JiraTracker } from " +
      JSON.stringify(TRACKER) +
      ";\n" +
      'console.log(JSON.stringify(await new JiraTracker("PAY-7", "acme/payments")[' +
      JSON.stringify(method) +
      "](..." +
      JSON.stringify(args) +
      ")));\n",
  );
  return run([process.execPath, script], {
    env: {
      ...fixture.env,
      JIRA_BASE_URL: BASE,
      JIRA_PAT: "pat-token",
      JIRA_EMAIL: undefined,
      JIRA_API_TOKEN: undefined,
      JIRA_API_VERSION: "3",
      EPIC_API_ATTEMPTS: "1",
      TOOLU_CONFIG_DIR: sb.path("config"),
      CLAUDE_CONFIG_DIR: sb.path("claude"),
      CODEX_HOME: sb.path("codex"),
      HOME: sb.root,
      ...env,
    },
  });
}

const issue = (key: string, summary: string, done = false) => ({
  key,
  fields: {
    summary,
    status: { statusCategory: { key: done ? "done" : "new" } },
    description: {
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "Repo: acme/refunds" }] }],
    },
    labels: ["backend"],
    issuelinks: [
      {
        type: { inward: "is blocked by", outward: "blocks" },
        inwardIssue: { key: "PAY-11", fields: { status: { statusCategory: { key: "new" } } } },
      },
    ],
  },
});

test.concurrent("v3 bearer client reads epic and paginated children with blockers", async () => {
  using sb = createSandbox();
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  try {
    fixture.plan([{ body: JSON.stringify(issue("PAY-7", "Refunds epic", true)) }]);
    const epic = await via(sb, fixture, "epic");
    expect(epic.exitCode).toBe(0);
    expect(JSON.parse(epic.stdout)).toEqual({
      ref: "PAY-7",
      title: "Refunds epic",
      state: "closed",
      url: BASE + "/browse/PAY-7",
    });
    expect(fixture.requests[0]).toMatchObject({
      method: "GET",
      path: "/rest/api/3/issue/PAY-7?fields=summary,status",
    });
    expect(fixture.requests[0]?.headers["authorization"]).toBe("Bearer pat-token");

    fixture.plan([
      {
        body: JSON.stringify({ issues: [issue("PAY-12", "Build refunds")], nextPageToken: "next" }),
      },
      { body: JSON.stringify({ issues: [issue("PAY-13", "Test refunds")] }) },
    ]);
    const children = await via(sb, fixture, "children");
    expect(children.exitCode).toBe(0);
    expect(JSON.parse(children.stdout)).toMatchObject([
      { ref: "PAY-12", repo: "acme/refunds", blockers: { "PAY-11": "open" } },
      { ref: "PAY-13", repo: "acme/refunds" },
    ]);
    expect(fixture.requests.map((r) => r.path)).toEqual([
      "/rest/api/3/search/jql",
      "/rest/api/3/search/jql",
    ]);
    expect(JSON.parse(fixture.requests[1]?.body ?? "{}")).toMatchObject({
      nextPageToken: "next",
      jql: "parent = PAY-7 ORDER BY key",
    });
  } finally {
    await fixture.stop();
  }
});

test.concurrent("v2 basic client pages children and comments then transitions by name", async () => {
  using sb = createSandbox();
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  const env = {
    JIRA_PAT: undefined,
    JIRA_EMAIL: "agent@example.com",
    JIRA_API_TOKEN: "api-token",
    JIRA_API_VERSION: "2",
  };
  try {
    fixture.plan([
      { body: JSON.stringify({ issues: [issue("PAY-12", "Build refunds")], total: 2 }) },
      { body: JSON.stringify({ issues: [issue("PAY-13", "Test refunds")], total: 2 }) },
    ]);
    const children = await via(sb, fixture, "children", [], env);
    expect(children.exitCode).toBe(0);
    expect(JSON.parse(children.stdout)).toHaveLength(2);
    expect(fixture.requests.map((r) => r.path)).toEqual([
      "/rest/api/2/search",
      "/rest/api/2/search",
    ]);
    expect(JSON.parse(fixture.requests[1]?.body ?? "{}")).toMatchObject({ startAt: 1 });
    expect(fixture.requests[0]?.headers["authorization"]).toBe(
      "Basic " + Buffer.from("agent@example.com:api-token").toString("base64"),
    );

    fixture.plan([
      { body: JSON.stringify(issue("PAY-12", "Build refunds")) },
      { body: "{}" },
      {
        body: JSON.stringify({
          transitions: [
            {
              id: "31",
              name: "Done",
              to: { statusCategory: { key: "done" } },
            },
          ],
        }),
      },
      {
        body: JSON.stringify({
          transitions: [
            {
              id: "31",
              name: "Done",
              to: { statusCategory: { key: "done" } },
            },
          ],
        }),
      },
      { body: "{}" },
    ]);
    const closed = await via(sb, fixture, "closeIssue", ["PAY-12", "Delivered"], env);
    expect(closed.exitCode).toBe(0);
    expect(JSON.parse(closed.stdout)).toBe("transitioned-done");
    expect(fixture.requests.map((r) => r.path)).toEqual([
      "/rest/api/2/issue/PAY-12?fields=status",
      "/rest/api/2/issue/PAY-12/comment",
      "/rest/api/2/issue/PAY-12/transitions",
      "/rest/api/2/issue/PAY-12/transitions",
      "/rest/api/2/issue/PAY-12/transitions",
    ]);
    expect(JSON.parse(fixture.requests[1]?.body ?? "{}")).toEqual({ body: "Delivered" });
    expect(JSON.parse(fixture.requests[4]?.body ?? "{}")).toEqual({ transition: { id: "31" } });
  } finally {
    await fixture.stop();
  }
});

test.concurrent("v3 comments use ADF and epic closes through the named Done transition", async () => {
  using sb = createSandbox();
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  try {
    const transitions = {
      transitions: [
        {
          id: "41",
          name: "Resolve",
          to: { statusCategory: { key: "done" } },
        },
      ],
    };
    fixture.plan([
      { body: "{}" },
      { body: JSON.stringify(transitions) },
      { body: JSON.stringify(transitions) },
      { body: "{}" },
    ]);
    const result = await via(sb, fixture, "closeEpic", ["Shipped"]);
    expect(result.exitCode).toBe(0);
    expect(JSON.parse(result.stdout)).toBe("transitioned-done");
    expect(fixture.requests.map((r) => r.method)).toEqual(["POST", "GET", "GET", "POST"]);
    expect(JSON.parse(fixture.requests[0]?.body ?? "{}")).toEqual({
      body: {
        type: "doc",
        version: 1,
        content: [{ type: "paragraph", content: [{ type: "text", text: "Shipped" }] }],
      },
    });
    expect(JSON.parse(fixture.requests[3]?.body ?? "{}")).toEqual({
      transition: { id: "41" },
    });
  } finally {
    await fixture.stop();
  }
});

test.concurrent("missing base or credentials fail before HTTP with named variables", async () => {
  using sb = createSandbox();
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  try {
    for (const env of [
      { JIRA_BASE_URL: undefined },
      { JIRA_PAT: undefined, JIRA_EMAIL: undefined, JIRA_API_TOKEN: undefined },
    ]) {
      fixture.plan([]);
      const result = await via(sb, fixture, "epic", [], env);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("JIRA_BASE_URL");
      expect(result.stderr).toContain("JIRA_PAT");
      expect(result.stderr).toContain("JIRA_EMAIL");
      expect(result.stderr).toContain("JIRA_API_TOKEN");
      expect(fixture.requests).toHaveLength(0);
    }
  } finally {
    await fixture.stop();
  }
});

test.concurrent("bad API version fails before HTTP", async () => {
  using sb = createSandbox();
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  try {
    const result = await via(sb, fixture, "epic", [], { JIRA_API_VERSION: "4" });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("JIRA_API_VERSION");
    expect(fixture.requests).toHaveLength(0);
  } finally {
    await fixture.stop();
  }
});
