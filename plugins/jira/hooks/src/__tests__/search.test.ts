/**
 * JQL search and pagination (ported from search.bats and paginate.bats),
 * driven by recorded real multi-page search responses.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { BASE, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());
beforeEach(() => h.fixture.plan([]));

test("search v3 POSTs JQL unmodified to /search/jql", async () => {
  h.respond("search-v3-p1.json");
  const run = await h.jira(["search", "-q", "project=ABC AND status=Open"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({
    method: "POST",
    url: `${BASE}/rest/api/3/search/jql`,
    body: '{"jql":"project=ABC AND status=Open","maxResults":50}',
  });
  expect(h.only().headers["content-type"]).toBe("application/json");
});

test("search v2 POSTs to /rest/api/2/search", async () => {
  h.respond("search-v2-p1.json");
  const run = await h.jira(["search", "project=ABC", "-n", "5"], {
    env: { JIRA_API_VERSION: "2" },
  });
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({
    url: `${BASE}/rest/api/2/search`,
    body: '{"jql":"project=ABC","maxResults":5}',
  });
});

test("search --lean projects key/summary/status", async () => {
  h.respond("search-v3-p1.json");
  const run = await h.jira(["--lean", "search", "-q", "x"]);
  expect(run.status).toBe(0);
  expect(JSON.parse(run.stdout)).toEqual({
    issues: [
      { key: "ABC-123", summary: "Login page throws 500 on empty password", status: "In Progress" },
      { key: "ABC-124", summary: "Logout redirect loop", status: "To Do" },
    ],
  });
});

test("search --all (v3) follows nextPageToken across pages", async () => {
  h.respond("search-v3-p1.json", "search-v3-p2.json");
  const run = await h.jira(["search", "-q", "x", "--all", "-n", "2"]);
  expect(run.status).toBe(0);
  expect(JSON.parse(run.stdout).map((issue: { key: string }) => issue.key)).toEqual([
    "ABC-123",
    "ABC-124",
    "ABC-130",
  ]);
  expect(h.fixture.requests.map((request) => request.body)).toEqual([
    '{"jql":"x","maxResults":2}',
    '{"jql":"x","maxResults":2,"nextPageToken":"CAEaBjEwMDAy"}',
  ]);
});

test("search --all (v2) follows startAt across pages", async () => {
  h.respond("search-v2-p1.json", "search-v2-p2.json");
  const run = await h.jira(["search", "-q", "x", "--all", "-n", "2"], {
    env: { JIRA_API_VERSION: "2" },
  });
  expect(run.status).toBe(0);
  expect(JSON.parse(run.stdout)).toHaveLength(3);
  expect(h.fixture.requests.map((request) => request.body)).toEqual([
    '{"jql":"x","maxResults":2,"startAt":0}',
    '{"jql":"x","maxResults":2,"startAt":2}',
  ]);
});

test("paginate: a single isLast page makes one request", async () => {
  h.respond("search-v3-p2.json");
  const run = await h.jira(["search", "-q", "x", "--all"]);
  expect(run.status).toBe(0);
  expect(JSON.parse(run.stdout)).toHaveLength(1);
  expect(h.fixture.requests).toHaveLength(1);
});

test("paginate: a failing page exits 1 with nothing on stdout", async () => {
  h.fixture.plan([{ status: 400, body: '{"errorMessages":["bad jql"]}' }]);
  const run = await h.jira(["search", "-q", "x", "--all"]);
  expect(run.status).toBe(1);
  expect(run.stdout).toBe("");
  expect(run.stderr).toBe(`jira: HTTP 400 from ${BASE}/rest/api/3/search/jql\n`);
});

test("search --fields includes a fields array in the body", async () => {
  h.respond("search-v3-p1.json");
  const run = await h.jira(["search", "-q", "x", "--fields", "summary,status"]);
  expect(run.status).toBe(0);
  expect(h.only().body).toBe('{"jql":"x","maxResults":50,"fields":["summary","status"]}');
});

test("search with no JQL exits 1", async () => {
  const run = await h.jira(["search"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira search");
});

test("search -n that is not a number exits 2 before any request", async () => {
  const run = await h.jira(["search", "-q", "x", "-n", "ten"]);
  expect(run.status).toBe(2);
  expect(run.stderr).toBe("jira: -n must be a number\n");
  expect(h.fixture.requests).toHaveLength(0);
});

test("search with a second bare argument is an unknown option", async () => {
  const run = await h.jira(["search", "x", "y"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toBe("search: unknown option 'y'\n");
});
