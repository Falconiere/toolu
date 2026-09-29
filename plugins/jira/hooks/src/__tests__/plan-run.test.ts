/**
 * The check runner (ported from plan-run.bats): the whoami probe, the two
 * writes (running, then the verdict), evidence capture and --step/--activity.
 * Checks run through `bash -c` inside a real git repo and call the real
 * bundle back as "$JIRA", which reaches the fixture replaying the recorded
 * issue.json, so `green` really means "the REST assertion held".
 */
import { afterAll, afterEach, beforeEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { planCli } from "../jira/plan-run.ts";
import { BUNDLE, FIXTURES, fixtureBody, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());

let repo = "";
let ledgerFile = "";
beforeEach(() => {
  h.fixture.plan([]);
  repo = realpathSync(mkdtempSync(join(tmpdir(), "jira-run-")));
  spawnSync("git", ["init", "-q", repo]);
  ledgerFile = join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json");
  // issue.json has key ABC-123 and status "In Progress": the doc's first step
  // asserts the key (green), the second asserts status Done (red).
  copyFileSync(join(FIXTURES, "plan-ok.md"), join(repo, "plan.md"));
});
afterEach(() => rmSync(repo, { recursive: true, force: true }));

function run(...args: string[]) {
  return h.jira(["plan", "run", ...args], { cwd: repo, env: { JIRA_CLI: BUNDLE } });
}

function ledger(): { steps: Array<Record<string, unknown>>; [key: string]: unknown } {
  return JSON.parse(readFileSync(ledgerFile, "utf8"));
}

function step(id: string): Record<string, unknown> | undefined {
  return ledger().steps.find((item) => item["id"] === id);
}

function planDoc(name: string, steps: unknown): string {
  const body = `# t\n\n**Issue:** ABC-123   **Topic:** t\n\n## Steps (machine-readable)\n\n\`\`\`json\n${JSON.stringify(steps)}\n\`\`\`\n`;
  writeFileSync(join(repo, name), body);
  return name;
}

test("plan CLI: explicit Codex host uses HOME/.codex without lifecycle variables", () => {
  expect(planCli({ HOME: "/home/me", TOOLU_HOST_OVERRIDE: "codex" })).toBe(
    "/home/me/.codex/jira/jira.sh",
  );
  expect(planCli({ HOME: "/home/me", CODEX_HOME: "/c", PLUGIN_ROOT: "/p" })).toBe(
    "/c/jira/jira.sh",
  );
  expect(planCli({ HOME: "/home/me" })).toBe("/home/me/.claude/jira/jira.sh");
  expect(planCli({ HOME: "/home/me", TOOLU_CONFIG_DIR: "/t", JIRA_CLI: "" })).toBe(
    "/t/jira/jira.sh",
  );
  expect(planCli({ JIRA_CLI: "/x/jira" })).toBe("/x/jira");
});

test("run: green when Jira agrees, red when it does not; exits non-zero", async () => {
  h.respond("issue.json");
  const result = await run("plan.md");
  expect(result.status).toBe(1);
  expect(result.stdout).toBe("green  comment-pr-link\nred    transition-done (exit 1)\n");
  expect(step("comment-pr-link")).toMatchObject({ status: "green", exit_code: 0 });
  expect(step("transition-done")).toMatchObject({ status: "red", exit_code: 1 });
});

test("run: writes a v1 ledger at jira-<KEY>.json with base_branch empty", async () => {
  h.respond("issue.json");
  await run("plan.md");
  expect(ledger()).toMatchObject({
    version: 1,
    branch: "jira-ABC-123",
    base_branch: "",
    plan_doc: "plan.md",
  });
});

test("run: summary and next reflect the verdicts", async () => {
  h.respond("issue.json");
  await run("plan.md");
  expect(ledger()).toMatchObject({
    summary: { green: 1, red: 1, stale: 0 },
    next: "transition-done",
  });
});

test("run: the step is status=running while its own check executes", async () => {
  // The check reads the ledger the runner just wrote: the first of the two
  // writes lands before the check runs (what the dashboard renders).
  const doc = planDoc("obs.md", [
    {
      id: "obs",
      title: "observe running",
      check:
        'jq -e \'.steps[]|select(.id=="obs")|.status=="running" and .started_at!=null\' .claude/tmp/plan-ledger/jira-ABC-123.json >/dev/null',
    },
  ]);
  h.respond("issue.json");
  const result = await run(doc);
  expect(result.status).toBe(0);
  expect(ledger().steps[0]).toMatchObject({ status: "green", started_at: null });
});

test("run: probe failure writes NO statuses and leaves an existing ledger untouched", async () => {
  h.respond("issue.json");
  await run("plan.md");
  const before = readFileSync(ledgerFile, "utf8");
  h.fixture.plan([{ status: 500, body: '{"errorMessages":["down"]}' }]);
  const result = await run("plan.md");
  expect(result.status).toBe(1);
  expect(result.stderr).toContain("cannot reach Jira");
  expect(result.stderr).toContain("no step statuses were written");
  expect(readFileSync(ledgerFile, "utf8")).toBe(before);
});

test("run: probe failure on a fresh repo creates no ledger at all", async () => {
  h.fixture.plan([{ status: 401, body: "{}" }]);
  expect((await run("plan.md")).status).toBe(1);
  expect(existsSync(ledgerFile)).toBe(false);
});

test("run: --step executes only that step, leaving the other pending", async () => {
  h.respond("issue.json");
  const result = await run("plan.md", "--step", "comment-pr-link");
  expect(result.status).toBe(0);
  expect(step("comment-pr-link")?.["status"]).toBe("green");
  expect(step("transition-done")).toMatchObject({ status: "pending", exit_code: null });
});

test("run: --step with an unknown id errors and writes nothing", async () => {
  h.respond("issue.json");
  const result = await run("plan.md", "--step", "nope");
  expect(result.status).toBe(1);
  expect(result.stderr).toBe("jira plan run: no step 'nope' in plan.md\n");
  expect(existsSync(ledgerFile)).toBe(false);
});

test("run: --activity labels the executed step", async () => {
  h.respond("issue.json");
  await run("plan.md", "--step", "comment-pr-link", "--activity", "checking the link");
  expect(step("comment-pr-link")?.["activity"]).toBe("checking the link");
});

test("run: a red step captures the check's output as evidence_tail", async () => {
  h.respond("issue.json");
  expect((await run("plan.md", "--step", "transition-done")).status).toBe(1);
  expect(typeof step("transition-done")?.["evidence_tail"]).toBe("string");
});

test("run: evidence keeps stdout and stderr interleaved, as 2>&1 did", async () => {
  const doc = planDoc("mixed.md", [
    { id: "mixed", title: "mixed output", check: "echo one; echo two >&2; echo three; exit 3" },
  ]);
  h.respond("issue.json");
  const result = await run(doc);
  expect(result.stdout).toBe("red    mixed (exit 3)\n");
  expect(ledger().steps[0]).toMatchObject({
    status: "red",
    exit_code: 3,
    evidence_tail: "one\ntwo\nthree",
  });
});

test("run: checks inherit the resolved API version through $JIRA", async () => {
  const doc = planDoc("v2.md", [
    { id: "v2", title: "v2 call", check: '"$JIRA" project list >/dev/null' },
  ]);
  const result = await h.jira(["--api-version", "2", "plan", "run", doc], {
    cwd: repo,
    env: { JIRA_CLI: BUNDLE },
  });
  expect(result.status).toBe(0);
  expect(h.urls()).toEqual([
    "https://acme.atlassian.net/rest/api/2/myself",
    "https://acme.atlassian.net/rest/api/2/project",
  ]);
});

test("run: an unknown option is rejected", async () => {
  const result = await run("plan.md", "--bogus", "x");
  expect(result.status).toBe(1);
  expect(result.stderr).toBe("jira plan run: unknown option '--bogus'\n");
});

test("run: a missing jira CLI is reported, not silently treated as red", async () => {
  const absent = join(repo, "absent.sh");
  const result = await h.jira(["plan", "run", "plan.md"], { cwd: repo, env: { JIRA_CLI: absent } });
  expect(result.status).toBe(1);
  expect(result.stderr).toBe(`jira plan run: jira CLI not found at ${absent} (set JIRA_CLI)\n`);
  expect(existsSync(ledgerFile)).toBe(false);
});

test("run: re-running a red step flips it green once Jira agrees", async () => {
  h.respond("issue.json");
  await run("plan.md", "--step", "transition-done");
  expect(step("transition-done")?.["status"]).toBe("red");
  const done = JSON.parse(fixtureBody("issue.json"));
  done.fields.status.name = "Done";
  h.fixture.plan([{ body: JSON.stringify(done) }]);
  const result = await run("plan.md", "--step", "transition-done");
  expect(result.status).toBe(0);
  expect(step("transition-done")).toMatchObject({ status: "green", exit_code: 0 });
  expect(ledger()).toMatchObject({ summary: { green: 1 }, next: "comment-pr-link" });
});
