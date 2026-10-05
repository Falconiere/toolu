/** Worker issue command against the real REST client and loopback HTTPS. */
import { expect, test } from "bun:test";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { run } from "@toolu/conformance/harness/spawn";
import { join } from "node:path";

const CLI = join(import.meta.dir, "../jira-issue.ts");
const BASE = "https://acme.atlassian.net";
const ISSUE = {
  key: "PAY-12",
  fields: {
    summary: "Build refunds",
    status: { name: "To Do" },
    description: {
      type: "doc",
      content: [{ type: "paragraph", content: [{ type: "text", text: "Build it" }] }],
    },
    issuelinks: [
      {
        type: { inward: "is blocked by", outward: "blocks" },
        inwardIssue: { key: "PAY-11" },
      },
    ],
  },
};

test.concurrent.each([
  [
    "3",
    { JIRA_PAT: "pat-token", JIRA_EMAIL: undefined, JIRA_API_TOKEN: undefined },
    "Bearer pat-token",
  ],
  [
    "2",
    { JIRA_PAT: undefined, JIRA_EMAIL: "agent@example.com", JIRA_API_TOKEN: "api-token" },
    "Basic " + Buffer.from("agent@example.com:api-token").toString("base64"),
  ],
])("get prints the full v%s Jira issue without a plugin", async (version, creds, auth) => {
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  try {
    fixture.plan([{ body: JSON.stringify(ISSUE) }]);
    const result = await run([process.execPath, CLI, "get", "PAY-12"], {
      env: {
        ...fixture.env,
        JIRA_BASE_URL: BASE,
        JIRA_API_VERSION: version,
        ...creds,
      },
    });
    expect(result.exitCode).toBe(0);
    expect(result.stderr).toBe("");
    expect(JSON.parse(result.stdout)).toEqual(ISSUE);
    expect(fixture.requests).toHaveLength(1);
    expect(fixture.requests[0]).toMatchObject({
      method: "GET",
      path: `/rest/api/${version}/issue/PAY-12`,
    });
    expect(fixture.requests[0]?.headers["authorization"]).toBe(auth);
  } finally {
    await fixture.stop();
  }
});

test.concurrent("get rejects missing credentials and bad arguments before HTTP", async () => {
  const fixture = await startHttpsFixture(["acme.atlassian.net"]);
  try {
    for (const args of [
      ["get", "PAY-12"],
      ["get", "../PAY-12"],
      ["delete", "PAY-12"],
    ]) {
      fixture.plan([]);
      const result = await run([process.execPath, CLI, ...args], {
        env: {
          ...fixture.env,
          JIRA_BASE_URL: BASE,
          JIRA_PAT: undefined,
          JIRA_EMAIL: undefined,
          JIRA_API_TOKEN: undefined,
        },
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain(
        args[1] === "PAY-12" && args[0] === "get" ? "JIRA_BASE_URL" : "Usage:",
      );
      expect(fixture.requests).toHaveLength(0);
    }
  } finally {
    await fixture.stop();
  }
});
