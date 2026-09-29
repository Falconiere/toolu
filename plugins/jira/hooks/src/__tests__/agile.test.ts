/**
 * Agile boards and sprints (ported from board.bats and sprint.bats): the
 * exact /rest/agile/1.0 URLs, query strings and bodies the bundle sends.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { BASE, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());
beforeEach(() => h.fixture.plan([]));

const AGILE = `${BASE}/rest/agile/1.0`;

test("board list hits /rest/agile/1.0/board", async () => {
  const run = await h.jira(["board", "list"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board`);
});

test("board list -p ABC includes projectKeyOrId=ABC; --lean projects id/name/type", async () => {
  h.fixture.plan([{ body: '{"values":[{"id":5,"name":"ABC board","type":"scrum","self":"x"}]}' }]);
  const run = await h.jira(["board", "list", "-p", "ABC", "--lean"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board?projectKeyOrId=ABC`);
  expect(JSON.parse(run.stdout)).toEqual({ values: [{ id: 5, name: "ABC board", type: "scrum" }] });
});

test("board get 5 hits /rest/agile/1.0/board/5", async () => {
  const run = await h.jira(["board", "get", "5"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board/5`);
});

test("board issues 5 hits /rest/agile/1.0/board/5/issue", async () => {
  const run = await h.jira(["board", "issues", "5"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board/5/issue`);
});

test("board issues 5 -q URL-encodes the jql in the query string", async () => {
  const run = await h.jira(["board", "issues", "5", "-q", "status=Done"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board/5/issue?jql=status%3DDone`);
});

test("board issues 5 -q URL-encodes spaces in a multi-word JQL", async () => {
  const run = await h.jira(["board", "issues", "5", "-q", "status = Done"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board/5/issue?jql=status%20%3D%20Done`);
});

test("board issues 5 -n includes maxResults in the query string", async () => {
  const run = await h.jira(["board", "issues", "5", "-q", "a", "-n", "10"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board/5/issue?jql=a&maxResults=10`);
});

test("board unknown action exits 1 with usage", async () => {
  const run = await h.jira(["board", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira board");
});

test("board get with no id exits 1 with usage", async () => {
  const run = await h.jira(["board", "get"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira board get");
});

test("board issues with no id exits 1 with usage", async () => {
  const run = await h.jira(["board", "issues"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira board issues");
});

test("sprint list hits the board sprint endpoint", async () => {
  const run = await h.jira(["sprint", "list", "5"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/board/5/sprint`);
});

test("sprint get hits the sprint endpoint", async () => {
  const run = await h.jira(["sprint", "get", "9"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/sprint/9`);
});

test("sprint issues hits the sprint issue endpoint", async () => {
  h.respond("search-v3-p2.json");
  const run = await h.jira(["sprint", "issues", "9", "--lean"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${AGILE}/sprint/9/issue`);
  expect(JSON.parse(run.stdout)).toEqual({
    issues: [{ key: "ABC-130", summary: "Session cookie not cleared on logout", status: "Done" }],
  });
});

test("sprint create POSTs originBoardId and name to the sprint endpoint", async () => {
  const run = await h.jira(["sprint", "create", "5", "-n", "Sprint 1"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({
    method: "POST",
    url: `${AGILE}/sprint`,
    body: '{"originBoardId":5,"name":"Sprint 1"}',
  });
});

test("sprint create missing name exits 1", async () => {
  const run = await h.jira(["sprint", "create", "5"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira sprint create");
});

test("sprint move POSTs the issue keys array", async () => {
  const run = await h.jira(["sprint", "move", "9", "ABC-1", "ABC-2"]);
  expect(run.status).toBe(0);
  expect(h.only()).toMatchObject({
    url: `${AGILE}/sprint/9/issue`,
    body: '{"issues":["ABC-1","ABC-2"]}',
  });
});

test("sprint move missing keys exits 1", async () => {
  const run = await h.jira(["sprint", "move", "9"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira sprint move");
});

test("sprint start POSTs active state to the sprint endpoint", async () => {
  h.fixture.plan([{ status: 204, body: "" }]);
  const run = await h.jira(["sprint", "start", "9"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("started sprint 9\n");
  expect(h.only()).toMatchObject({
    method: "POST",
    url: `${AGILE}/sprint/9`,
    body: '{"state":"active"}',
  });
});

test("sprint complete POSTs closed state to the sprint endpoint", async () => {
  h.fixture.plan([{ status: 204, body: "" }]);
  const run = await h.jira(["sprint", "complete", "9"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("completed sprint 9\n");
  expect(h.only()).toMatchObject({ url: `${AGILE}/sprint/9`, body: '{"state":"closed"}' });
});

test("sprint unknown action exits 1", async () => {
  const run = await h.jira(["sprint", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira sprint <list|get|issues|create|move|start|complete>");
});

test("sprint create with a non-numeric board id exits 2 before any request", async () => {
  const run = await h.jira(["sprint", "create", "five", "-n", "S"]);
  expect(run.status).toBe(2);
  expect(h.fixture.requests).toHaveLength(0);
});

test("sprint create with a non-integer board id exits 2 before any request", async () => {
  const run = await h.jira(["sprint", "create", "123.45", "-n", "S"]);
  expect(run.status).toBe(2);
  expect(run.stderr).toBe("jira: BOARD_ID must be an integer\n");
  expect(h.fixture.requests).toHaveLength(0);
});
