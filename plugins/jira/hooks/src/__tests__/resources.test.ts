/**
 * Projects, users and worklogs (ported from project.bats, user.bats and
 * worklog.bats): endpoint shapes, version gating, encoding, lean views and
 * argument guards. One live test runs only with JIRA_LIVE=1 and real creds.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { BASE, BUNDLE, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());
beforeEach(() => h.fixture.plan([]));

const API3 = `${BASE}/rest/api/3`;

test("project list hits the v3 project collection; --lean keeps key/name/id", async () => {
  h.fixture.plan([{ body: '[{"key":"ABC","name":"Alpha","id":"1","lead":{}}]' }]);
  const run = await h.jira(["project", "list", "--lean"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({ method: "GET", url: `${API3}/project` });
  expect(JSON.parse(run.stdout)).toEqual({ projects: [{ key: "ABC", name: "Alpha", id: "1" }] });
});

test("project get <KEY> hits the project detail endpoint", async () => {
  expect((await h.jira(["project", "get", "ABC"])).status).toBe(0);
  expect(h.only().url).toBe(`${API3}/project/ABC`);
});

test("project versions <KEY> hits the versions endpoint", async () => {
  expect((await h.jira(["project", "versions", "ABC"])).status).toBe(0);
  expect(h.only().url).toBe(`${API3}/project/ABC/versions`);
});

test("project components <KEY> hits the components endpoint", async () => {
  expect((await h.jira(["project", "components", "ABC"])).status).toBe(0);
  expect(h.only().url).toBe(`${API3}/project/ABC/components`);
});

test("project list honors JIRA_API_VERSION=2", async () => {
  expect((await h.jira(["project", "list"], { env: { JIRA_API_VERSION: "2" } })).status).toBe(0);
  expect(h.only().url).toBe(`${BASE}/rest/api/2/project`);
});

test("project get with no key exits 1", async () => {
  const run = await h.jira(["project", "get"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira project get");
});

test("project unknown action exits 1 with usage", async () => {
  const run = await h.jira(["project", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira project <list|get|versions|components>");
});

test("user whoami hits /rest/api/3/myself; --lean keeps the identity", async () => {
  h.fixture.plan([{ body: '{"accountId":"5b10","displayName":"Mia","active":true}' }]);
  const run = await h.jira(["user", "whoami", "--lean"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${API3}/myself`);
  expect(JSON.parse(run.stdout)).toEqual({
    accountId: "5b10",
    displayName: "Mia",
    emailAddress: null,
  });
});

test("user search -q (v3) hits /user/search?query=bob", async () => {
  expect((await h.jira(["user", "search", "-q", "bob"])).status).toBe(0);
  expect(h.only().url).toBe(`${API3}/user/search?query=bob`);
});

test("user search -q (v2) hits /user/search?username=bob", async () => {
  const run = await h.jira(["user", "search", "-q", "bob"], { env: { JIRA_API_VERSION: "2" } });
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${BASE}/rest/api/2/user/search?username=bob`);
});

test("user get (v3) hits /user?accountId=5b10", async () => {
  expect((await h.jira(["user", "get", "5b10"])).status).toBe(0);
  expect(h.only().url).toBe(`${API3}/user?accountId=5b10`);
});

test("user search URL-encodes a spaced query", async () => {
  expect((await h.jira(["user", "search", "-q", "John Doe"])).status).toBe(0);
  expect(h.only().url).toBe(`${API3}/user/search?query=John%20Doe`);
});

test("user search with no -q exits 1", async () => {
  const run = await h.jira(["user", "search"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira user search");
});

test("user unknown action exits 1", async () => {
  const run = await h.jira(["user", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira user <whoami|search|get>");
});

test.skipIf(process.env["JIRA_LIVE"] !== "1")("user whoami (live, gated)", async () => {
  // Real env, real Jira: no fixture proxy.
  const child = Bun.spawn([BUNDLE, "user", "whoami"], { stdout: "pipe", stderr: "pipe" });
  const [stdout, status] = await Promise.all([new Response(child.stdout).text(), child.exited]);
  expect(status).toBe(0);
  expect(JSON.parse(stdout)).toHaveProperty("accountId");
});

test("worklog add POSTs timeSpent to the worklog endpoint", async () => {
  expect((await h.jira(["worklog", "add", "ABC-1", "-t", "1h"])).status).toBe(0);
  expect(h.only()).toMatchObject({
    url: `${API3}/issue/ABC-1/worklog`,
    body: '{"timeSpent":"1h"}',
  });
});

test("worklog add wraps comment as ADF under v3", async () => {
  expect((await h.jira(["worklog", "add", "ABC-1", "-t", "1h", "-c", "did work"])).status).toBe(0);
  expect(JSON.parse(h.only().body)).toEqual({
    timeSpent: "1h",
    comment: {
      type: "doc",
      version: 1,
      content: [{ type: "paragraph", content: [{ type: "text", text: "did work" }] }],
    },
  });
});

test("worklog add uses a plain comment string under v2", async () => {
  const run = await h.jira(["worklog", "add", "ABC-1", "--time", "1h", "--comment", "did work"], {
    env: { JIRA_API_VERSION: "2" },
  });
  expect(run.status).toBe(0);
  expect(h.only().body).toBe('{"timeSpent":"1h","comment":"did work"}');
});

test("worklog add with no -t exits 1", async () => {
  const run = await h.jira(["worklog", "add", "ABC-1"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira worklog add");
});

test("worklog list GETs the worklog endpoint; --lean projects each entry", async () => {
  h.fixture.plan([
    {
      body: '{"worklogs":[{"id":"99","author":{"displayName":"Mia"},"timeSpent":"2h","started":"2026-01-01T00:00:00.000+0000","comment":"x"}]}',
    },
  ]);
  const run = await h.jira(["worklog", "list", "ABC-1", "--lean"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({ method: "GET", url: `${API3}/issue/ABC-1/worklog` });
  expect(JSON.parse(run.stdout)).toEqual({
    worklogs: [
      { id: "99", author: "Mia", timeSpent: "2h", started: "2026-01-01T00:00:00.000+0000" },
    ],
  });
});

test("worklog delete DELETEs the worklog entry", async () => {
  h.fixture.plan([{ status: 204, body: "" }]);
  const run = await h.jira(["worklog", "delete", "ABC-1", "99"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({ method: "DELETE", url: `${API3}/issue/ABC-1/worklog/99` });
  expect(run.stdout).toBe("deleted worklog 99\n");
});

test("worklog with unknown action exits 1", async () => {
  const run = await h.jira(["worklog", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira worklog <add|list|delete>");
});
