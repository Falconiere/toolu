/**
 * The dispatcher, transport and raw family (ported from dispatch.bats,
 * http.bats and raw.bats): global flags, auth, base URL, API version, the
 * HTTP error contract and the escape hatch, all through the real bundle.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { BASE, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());
beforeEach(() => h.fixture.plan([]));

test("dispatch: no args prints usage and exits 1", async () => {
  const run = await h.jira([]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira");
  expect(run.stderr).toContain("Families:");
  expect(run.stdout).toBe("");
});

test("dispatch: unknown family exits 1 naming it", async () => {
  const run = await h.jira(["bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toStartWith("jira: unknown family 'bogus'\n");
  expect(run.stderr).toContain("Usage: jira");
});

test("dispatch: --api-version <n> is consumed, not treated as family", async () => {
  const run = await h.jira(["--api-version", "2", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("unknown family 'bogus'");
});

test("dispatch: --api-version=<n> form is consumed", async () => {
  const run = await h.jira(["--api-version=2", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("unknown family 'bogus'");
});

test("dispatch: --lean alone (no family) prints usage", async () => {
  const run = await h.jira(["--lean"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira");
});

test("dispatch: a global flag after the family is still stripped", async () => {
  const run = await h.jira(["bogus", "--lean"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("unknown family 'bogus'");
});

test("dispatch: --api-version with no value exits 1 naming the flag", async () => {
  const run = await h.jira(["user", "whoami", "--api-version"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toBe("jira: --api-version needs a value\n");
  expect(h.fixture.requests).toHaveLength(0);
});

test("http: missing credentials exits 1 naming the vars, no request", async () => {
  const run = await h.jira(["user", "whoami"], { env: { JIRA_PAT: undefined } });
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("JIRA_PAT");
  expect(run.stderr).toContain("JIRA_EMAIL");
  expect(h.fixture.requests).toHaveLength(0);
});

test("http: JIRA_BASE_URL unset exits 1 naming it, no request", async () => {
  const run = await h.jira(["user", "whoami"], { env: { JIRA_BASE_URL: undefined } });
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("JIRA_BASE_URL");
  expect(h.fixture.requests).toHaveLength(0);
});

test("http: JIRA_PAT yields Bearer auth", async () => {
  const run = await h.jira(["raw", "GET", "/rest/api/3/issue/ABC-1"]);
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request.headers["authorization"]).toBe("Bearer tok");
  expect(request.headers["accept"]).toBe("application/json");
});

test("http: email+token yields basic auth and no Bearer header", async () => {
  const run = await h.jira(["raw", "GET", "/rest/api/3/issue/ABC-1"], {
    env: { JIRA_PAT: undefined, JIRA_EMAIL: "me@x.com", JIRA_API_TOKEN: "token" },
  });
  expect(run.status).toBe(0);
  const auth = h.only().headers["authorization"] ?? "";
  expect(auth).toBe(`Basic ${Buffer.from("me@x.com:token").toString("base64")}`);
});

test("http: trailing slash on base URL is stripped (no double slash)", async () => {
  const run = await h.jira(["user", "whoami"], { env: { JIRA_BASE_URL: `${BASE}//` } });
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${BASE}/rest/api/3/myself`);
});

test("http: default api version is 3", async () => {
  expect((await h.jira(["project", "list"])).status).toBe(0);
  expect(h.only().url).toBe(`${BASE}/rest/api/3/project`);
});

test("http: JIRA_API_VERSION=2 is honored, and --api-version overrides it", async () => {
  expect((await h.jira(["project", "list"], { env: { JIRA_API_VERSION: "2" } })).status).toBe(0);
  expect(h.only().url).toBe(`${BASE}/rest/api/2/project`);
  h.fixture.plan([]);
  const run = await h.jira(["--api-version", "3", "project", "list"], {
    env: { JIRA_API_VERSION: "2" },
  });
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${BASE}/rest/api/3/project`);
});

test("http: invalid api version exits 1", async () => {
  const run = await h.jira(["user", "whoami"], { env: { JIRA_API_VERSION: "9" } });
  expect(run.status).toBe(1);
  expect(run.stderr).toBe("jira: api version must be 2 or 3 (got '9')\n");
  expect(h.fixture.requests).toHaveLength(0);
});

test("raw: GET hits the exact path with resolved auth and no body", async () => {
  const run = await h.jira(["raw", "GET", "/rest/api/3/myself"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("{}\n");
  const request = h.only();
  expect(request).toMatchObject({ method: "GET", url: `${BASE}/rest/api/3/myself`, body: "" });
  expect(request.headers["authorization"]).toBe("Bearer tok");
  expect(request.headers["content-type"]).toBeUndefined();
});

test("raw: POST sends the body verbatim", async () => {
  const run = await h.jira(["raw", "POST", "/x", '{"a": 1}']);
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request).toMatchObject({ method: "POST", url: `${BASE}/x`, body: '{"a": 1}' });
  expect(request.headers["content-type"]).toBe("application/json");
});

test("raw: an HTTP failure exits 22 with the error body on stdout", async () => {
  h.fixture.plan([{ status: 401, body: '{"errorMessages":["stub failure"]}' }]);
  const run = await h.jira(["raw", "GET", "/rest/api/3/myself"]);
  expect(run.status).toBe(22);
  expect(run.stdout).toBe('{\n  "errorMessages": [\n    "stub failure"\n  ]\n}\n');
  expect(run.stderr).toBe(`jira: HTTP 401 from ${BASE}/rest/api/3/myself\n`);
});

test("raw: too few args exits 1 with usage", async () => {
  const run = await h.jira(["raw", "GET"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira raw");
});

test("--lean with an HTTP error prints the real error body, not a projection", async () => {
  h.fixture.plan([{ status: 404, body: '{"errorMessages":["Issue does not exist"]}' }]);
  const run = await h.jira(["issue", "get", "ABC-9", "--lean"]);
  expect(run.status).toBe(22);
  expect(JSON.parse(run.stdout)).toEqual({ errorMessages: ["Issue does not exist"] });
});

test("a non-JSON success body exits 5", async () => {
  h.fixture.plan([{ body: "<html>maintenance</html>", contentType: "text/html" }]);
  const run = await h.jira(["project", "list"]);
  expect(run.status).toBe(5);
  expect(run.stderr).toBe("jira: response is not JSON\n");
});

test("an empty success body prints nothing", async () => {
  h.fixture.plan([{ status: 204, body: "" }]);
  const run = await h.jira(["raw", "DELETE", "/rest/api/3/version/1"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("");
  expect(h.only().method).toBe("DELETE");
});

test("a dropped connection exits 1 with a request-failed message", async () => {
  h.fixture.plan([{ close: true }]);
  const run = await h.jira(["user", "whoami"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toStartWith("jira: request failed: ");
});
