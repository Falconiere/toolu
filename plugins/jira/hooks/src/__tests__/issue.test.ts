/**
 * The issue family (ported from issue.bats and adf.bats): read with and
 * without --lean over a recorded real issue, create/update/delete, comments
 * with version-gated ADF bodies, transitions by name or id, and assignment.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { BASE, fixtureBody, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());
beforeEach(() => h.fixture.plan([]));

const ISSUE = `${BASE}/rest/api/3/issue`;
const adf = (text: string) => ({
  type: "doc",
  version: 1,
  content: [{ type: "paragraph", content: [{ type: "text", text }] }],
});

test("issue get --lean projects and drops null/empty fields", async () => {
  h.respond("issue.json");
  const run = await h.jira(["issue", "get", "ABC-123", "--lean"]);
  expect(run.status).toBe(0);
  const lean = JSON.parse(run.stdout);
  expect(lean).toMatchObject({ key: "ABC-123", status: "In Progress", assignee: "Mia Krystof" });
  expect(lean).not.toHaveProperty("resolution");
  expect(lean).not.toHaveProperty("components");
  expect(Object.values(lean)).not.toContain(null);
});

test("issue get (non-lean) returns the full body, laid out like jq", async () => {
  h.respond("issue.json");
  const run = await h.jira(["issue", "get", "ABC-123"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe(`${JSON.stringify(JSON.parse(fixtureBody("issue.json")), null, 2)}\n`);
  expect(h.only()).toMatchObject({ method: "GET", url: `${ISSUE}/ABC-123` });
});

test("issue get with no key exits 1", async () => {
  const run = await h.jira(["issue", "get"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira issue get");
});

test("issue create POSTs project/type/summary to the issue endpoint", async () => {
  const run = await h.jira(["issue", "create", "-p", "ABC", "-t", "Bug", "-s", "boom"]);
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request).toMatchObject({ method: "POST", url: ISSUE });
  expect(request.body).toBe(
    '{"fields":{"project":{"key":"ABC"},"issuetype":{"name":"Bug"},"summary":"boom"}}',
  );
});

test("issue create adds description, assignee and -f fields in bash's order", async () => {
  const run = await h.jira([
    "issue",
    "create",
    "-f",
    "priority=High",
    "--project",
    "ABC",
    "--type",
    "Task",
    "--summary",
    "s",
    "-d",
    "why",
    "--assignee",
    "5b10",
    "--field",
    "labels",
  ]);
  expect(run.status).toBe(0);
  expect(JSON.parse(h.only().body)).toEqual({
    fields: {
      project: { key: "ABC" },
      issuetype: { name: "Task" },
      summary: "s",
      description: adf("why"),
      assignee: { accountId: "5b10" },
      priority: "High",
      labels: "labels",
    },
  });
});

test("issue create missing required option exits 1", async () => {
  const run = await h.jira(["issue", "create", "-p", "ABC"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira issue create");
  expect(h.fixture.requests).toHaveLength(0);
});

test("issue update PUTs changed fields to the issue endpoint", async () => {
  h.fixture.plan([{ status: 204, body: "" }]);
  const run = await h.jira(["issue", "update", "ABC-1", "-s", "new title", "-f", "priority=High"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("updated ABC-1\n");
  const request = h.only();
  expect(request).toMatchObject({ method: "PUT", url: `${ISSUE}/ABC-1` });
  expect(request.body).toBe('{"fields":{"summary":"new title","priority":"High"}}');
});

test("issue update with no key exits 1", async () => {
  const run = await h.jira(["issue", "update"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira issue update");
});

test("issue comment wraps text as ADF under v3", async () => {
  const run = await h.jira(["issue", "comment", "ABC-1", "hi there"]);
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request.url).toBe(`${ISSUE}/ABC-1/comment`);
  expect(JSON.parse(request.body)).toEqual({ body: adf("hi there") });
});

test("issue comment uses a plain body under v2", async () => {
  const run = await h.jira(["issue", "comment", "ABC-1", "hi"], { env: { JIRA_API_VERSION: "2" } });
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request.body).toBe('{"body":"hi"}');
  expect(request.url).toBe(`${BASE}/rest/api/2/issue/ABC-1/comment`);
});

test("adf: v3 body is the minimal ADF doc, byte-identical to bash's jq -c", async () => {
  await h.jira(["issue", "comment", "ABC-1", "hello world"]);
  expect(h.only().body).toBe(
    '{"body":{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"hello world"}]}]}}',
  );
});

test("adf: default version (no JIRA_API_VERSION) renders v3 ADF", async () => {
  await h.jira(["issue", "comment", "ABC-1", "x"], { env: { JIRA_API_VERSION: undefined } });
  expect(JSON.parse(h.only().body)).toEqual({ body: adf("x") });
});

test("adf: quotes and backslashes are JSON-escaped (v3 and v2)", async () => {
  const text = 'a " b \\ c';
  await h.jira(["issue", "comment", "ABC-1", text]);
  expect(JSON.parse(h.only().body)).toEqual({ body: adf(text) });
  h.fixture.plan([]);
  await h.jira(["--api-version", "2", "issue", "comment", "ABC-1", text]);
  expect(h.only().body).toBe('{"body":"a \\" b \\\\ c"}');
});

test("adf: a v2 description is the plain JSON string", async () => {
  const run = await h.jira(["issue", "update", "ABC-1", "-d", "plain"], {
    env: { JIRA_API_VERSION: "2" },
  });
  expect(run.status).toBe(0);
  expect(h.only().body).toBe('{"fields":{"description":"plain"}}');
});

test("issue transitions lists available transitions via GET", async () => {
  h.respond("transitions.json");
  const run = await h.jira(["issue", "transitions", "ABC-1", "--lean"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({ method: "GET", url: `${ISSUE}/ABC-1/transitions` });
  expect(JSON.parse(run.stdout)).toEqual({
    transitions: [
      { id: "11", name: "To Do" },
      { id: "21", name: "In Progress" },
      { id: "31", name: "Done" },
    ],
  });
});

test("issue transition resolves name to id (case-insensitive) then POSTs it", async () => {
  h.fixture.plan([{ body: fixtureBody("transitions.json") }, { status: 204, body: "" }]);
  const run = await h.jira(["issue", "transition", "ABC-1", "in progress"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("transitioned ABC-1 -> 21\n");
  expect(h.urls()).toEqual([`${ISSUE}/ABC-1/transitions`, `${ISSUE}/ABC-1/transitions`]);
  expect(h.fixture.requests[1]).toMatchObject({
    method: "POST",
    body: '{"transition":{"id":"21"}}',
  });
});

test("issue transition by numeric id POSTs it without a lookup", async () => {
  const run = await h.jira(["issue", "transition", "ABC-1", "31"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({ method: "POST", body: '{"transition":{"id":"31"}}' });
});

test("issue transition with no match exits 1 listing options", async () => {
  h.respond("transitions.json");
  const run = await h.jira(["issue", "transition", "ABC-1", "Nope"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toBe(
    "jira: no transition matches 'Nope'. Available: To Do, In Progress, Done\n",
  );
});

test("issue transition with an ambiguous name exits 1 without POSTing", async () => {
  h.respond("transitions-dup.json");
  const run = await h.jira(["issue", "transition", "ABC-1", "Done"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toBe("jira: transition 'Done' is ambiguous (matched ids: 11, 41)\n");
  expect(h.fixture.requests.map((request) => request.method)).toEqual(["GET"]);
});

test("issue transition lookup failing on HTTP exits 1 with no stdout", async () => {
  h.fixture.plan([{ status: 404, body: '{"errorMessages":["nope"]}' }]);
  const run = await h.jira(["issue", "transition", "ABC-1", "Done"]);
  expect(run.status).toBe(1);
  expect(run.stdout).toBe("");
  expect(run.stderr).toBe(`jira: HTTP 404 from ${ISSUE}/ABC-1/transitions\n`);
});

test("issue assign sends accountId under v3", async () => {
  const run = await h.jira(["issue", "assign", "ABC-1", "5b10acc"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({
    method: "PUT",
    url: `${ISSUE}/ABC-1/assignee`,
    body: '{"accountId":"5b10acc"}',
  });
});

test("issue assign sends name under v2", async () => {
  const run = await h.jira(["issue", "assign", "ABC-1", "jdoe"], {
    env: { JIRA_API_VERSION: "2" },
  });
  expect(run.status).toBe(0);
  expect(h.only().body).toBe('{"name":"jdoe"}');
});

test("issue assign - unassigns with null", async () => {
  h.fixture.plan([{ status: 204, body: "" }]);
  const run = await h.jira(["issue", "assign", "ABC-1", "-"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("assigned ABC-1\n");
  expect(h.only().body).toBe('{"accountId":null}');
});

test("issue delete DELETEs the issue", async () => {
  h.fixture.plan([{ status: 204, body: "" }]);
  const run = await h.jira(["issue", "delete", "ABC-1"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("deleted ABC-1\n");
  expect(h.only()).toMatchObject({ method: "DELETE", url: `${ISSUE}/ABC-1`, body: "" });
});

test("issue delete with no key exits 1", async () => {
  const run = await h.jira(["issue", "delete"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira issue delete");
});

test("issue delete that fails prints the raw error body and exits 22", async () => {
  h.fixture.plan([{ status: 403, body: '{"errorMessages":["no"]}' }]);
  const run = await h.jira(["issue", "delete", "ABC-1"]);
  expect(run.status).toBe(22);
  expect(run.stdout).toBe('{"errorMessages":["no"]}');
});

test("issue with an unknown action exits 1 with usage", async () => {
  const run = await h.jira(["issue", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain(
    "Usage: jira issue <get|create|update|delete|comment|transition|transitions|assign>",
  );
});
