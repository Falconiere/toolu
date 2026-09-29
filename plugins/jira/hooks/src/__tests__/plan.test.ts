/**
 * The plan family through the real bundle (ported from plan.bats,
 * plan-init.bats and plan-contract.bats): routing and usage, the credential
 * exemption for `plan path`, key validation, `init` scaffolding titled from a
 * live issue read, `status`, and the ledger contract the dashboard and push
 * gate rely on. Everything runs inside a real git repository.
 */
import { afterAll, afterEach, beforeEach, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { issueKey, parseSteps } from "../jira/plan-parse.ts";
import { BUNDLE, FIXTURES, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());

let sandbox = "";
let repo = "";
beforeEach(() => {
  h.fixture.plan([]);
  sandbox = realpathSync(mkdtempSync(join(tmpdir(), "jira-plan-")));
  repo = join(sandbox, "repo");
  mkdirSync(repo);
  spawnSync("git", ["init", "-q", repo]);
});
afterEach(() => rmSync(sandbox, { recursive: true, force: true }));

function plan(args: readonly string[], env: Readonly<Record<string, string | undefined>> = {}) {
  return h.jira(["plan", ...args], { cwd: repo, env: { JIRA_CLI: BUNDLE, ...env } });
}

const LEDGER = () => join(repo, ".claude/tmp/plan-ledger/jira-ABC-123.json");
const DOC = () => join(repo, ".claude/tmp/jira/plans/ABC-123.md");
const NO_CREDS = { JIRA_PAT: undefined, JIRA_BASE_URL: undefined };

describe("plan family", () => {
  test("dispatch: plan is a registered family and prints its own usage", async () => {
    const run = await plan([]);
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("jira plan — decompose ticket work");
    expect(run.stderr).not.toContain("unknown family");
  });

  test("dispatch: plan appears in the top-level usage banner", async () => {
    const run = await h.jira([]);
    expect(run.status).toBe(1);
    expect(run.stderr).toMatch(/plan\s+init\|run\|status\|path/);
  });

  test("plan path: prints the ledger path with NO credentials configured", async () => {
    const run = await plan(["path", "ABC-123"], NO_CREDS);
    expect(run.status).toBe(0);
    expect(run.stdout).toBe(`${LEDGER()}\n`);
  });

  test("plan path: still requires a key", async () => {
    const run = await plan(["path"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toBe("jira plan path: needs an issue key\n");
  });

  test("plan path: a traversal key from argv is rejected, not interpolated", async () => {
    const run = await plan(["path", "../../etc/passwd"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("not a valid issue key");
    expect(run.stdout).not.toContain("plan-ledger/jira-");
  });

  test("plan init: a traversal key from argv writes no file outside the plans dir", async () => {
    h.respond("issue.json");
    const run = await plan(["init", "../../../pwned"]);
    expect(run.status).toBe(1);
    expect(existsSync(join(sandbox, "pwned.md"))).toBe(false);
    expect(h.fixture.requests).toHaveLength(0);
  });

  test("credential gate still applies to other plan actions", async () => {
    const run = await plan(["status", "ABC-123"], NO_CREDS);
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("one-time setup step");
    expect(run.stdout).toBe("");
  });

  test("plan run: routes through the family, deriving the key from the doc", async () => {
    copyFileSync(join(FIXTURES, "plan-ok.md"), join(repo, "plan.md"));
    h.respond("issue.json");
    const run = await plan(["run", "plan.md", "--step", "comment-pr-link"]);
    expect(run.status).toBe(0);
    const ledger = JSON.parse(readFileSync(LEDGER(), "utf8"));
    expect(ledger.steps.find((step: { id: string }) => step.id === "comment-pr-link").status).toBe(
      "green",
    );
  });

  test("plan run: a doc with no **Issue:** header is rejected", async () => {
    writeFileSync(
      join(repo, "nokey.md"),
      '# x\n\n## Steps (machine-readable)\n\n```json\n[{"id":"a","title":"a","check":"true"}]\n```\n',
    );
    const run = await plan(["run", "nokey.md"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("missing a valid '**Issue:** <KEY>' header");
  });

  test("plan run: needs a doc path", async () => {
    const run = await plan(["run"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toBe("jira plan run: needs a plan doc path\n");
  });

  test("plan status: reports the summary of a written ledger", async () => {
    copyFileSync(join(FIXTURES, "plan-ok.md"), join(repo, "plan.md"));
    h.respond("issue.json");
    await plan(["run", "plan.md"]);
    const run = await plan(["status", "ABC-123"]);
    expect(run.status).toBe(0);
    expect(run.stdout).toBe(
      "jira-ABC-123  1/2 green   next: transition-done\n" +
        "  green\tcomment-pr-link\tComment the PR link on ABC-123\n" +
        "  red\ttransition-done\tMove ABC-123 to Done\n",
    );
  });

  test("plan status: no ledger yet is an error, not an empty success", async () => {
    const run = await plan(["status", "ZZZ-9"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toBe(
      "jira plan status: no ledger for ZZZ-9 (run: jira.sh plan run <doc>)\n",
    );
  });

  test("plan: an unknown action prints usage", async () => {
    const run = await plan(["bogus"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("jira plan — decompose");
  });
});

describe("plan init", () => {
  test("init: writes the doc under .claude/tmp/jira/plans and prints its path", async () => {
    h.respond("issue.json");
    const run = await plan(["init", "ABC-123"]);
    expect(run.status).toBe(0);
    expect(run.stdout).toBe(`${DOC()}\n`);
    expect(existsSync(DOC())).toBe(true);
    expect(h.only().url).toBe("https://acme.atlassian.net/rest/api/3/issue/ABC-123");
  });

  test("init: titles the doc from the live issue summary", async () => {
    h.respond("issue.json");
    await plan(["init", "ABC-123"]);
    expect(readFileSync(DOC(), "utf8").split("\n")[0]).toBe(
      "# ABC-123 — Login page throws 500 on empty password",
    );
  });

  test("init: still titles from the summary under --lean", async () => {
    h.respond("issue.json");
    expect((await h.jira(["--lean", "plan", "init", "ABC-123"], { cwd: repo })).status).toBe(0);
    expect(readFileSync(DOC(), "utf8")).toContain(
      "**Topic:** Login page throws 500 on empty password",
    );
  });

  test("init: emits the **Issue:** header the runner parses back", async () => {
    h.respond("issue.json");
    await plan(["init", "ABC-123"]);
    expect(issueKey(DOC())).toBe("ABC-123");
    expect(readFileSync(DOC(), "utf8")).toMatch(
      /^\*\*Date:\*\* \d{4}-\d\d-\d\d {3}\*\*Issue:\*\* ABC-123 {3}\*\*Topic:\*\* /m,
    );
  });

  test("init: scaffolds an empty machine-readable steps block", async () => {
    h.respond("issue.json");
    await plan(["init", "ABC-123"]);
    const lines = readFileSync(DOC(), "utf8").split("\n");
    expect(lines).toContain("## Steps (machine-readable)");
    expect(lines).toContain("[]");
  });

  test("init: the scaffolded empty block is rejected by the parser until filled", async () => {
    h.respond("issue.json");
    await plan(["init", "ABC-123"]);
    expect(() => parseSteps(DOC())).toThrow("not a non-empty array");
  });

  test("init: guidance shows a REST-assertion check bound to $JIRA", async () => {
    h.respond("issue.json");
    await plan(["init", "ABC-123"]);
    const doc = readFileSync(DOC(), "utf8");
    expect(doc).toContain(`"$JIRA" issue get ABC-123 --lean | jq -e '.status=="Done"' >/dev/null`);
    expect(doc).toContain("exits 0 **only when Jira itself reflects the change**");
    expect(doc).toContain(`Then: jira.sh plan run ${DOC()}\n`);
  });

  test("init: refuses to clobber an existing doc", async () => {
    h.respond("issue.json");
    await plan(["init", "ABC-123"]);
    writeFileSync(DOC(), "hand-edited\n");
    const run = await plan(["init", "ABC-123"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toBe(`jira plan init: ${DOC()} already exists\n`);
    expect(readFileSync(DOC(), "utf8")).toBe("hand-edited\n");
  });

  test("init: never writes through a symlink that sits at the doc path", async () => {
    h.respond("issue.json");
    mkdirSync(join(repo, ".claude/tmp/jira/plans"), { recursive: true });
    const target = join(sandbox, "outside.md");
    symlinkSync(target, DOC());
    const run = await plan(["init", "ABC-123"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toBe(`jira plan init: ${DOC()} already exists\n`);
    expect(existsSync(target)).toBe(false);
  });

  test("init: needs an issue key", async () => {
    const run = await plan(["init"]);
    expect(run.status).toBe(1);
    expect(run.stderr).toBe("jira plan init: needs an issue key\n");
  });

  test("init: a failed issue read exits 1 and writes no doc", async () => {
    h.fixture.plan([{ status: 404, body: '{"errorMessages":["Issue does not exist"]}' }]);
    const run = await plan(["init", "ABC-123"]);
    expect(run.status).toBe(1);
    expect(run.stdout).toBe("");
    expect(existsSync(DOC())).toBe(false);
  });

  test("init: end-to-end — scaffold, author a step, run it green", async () => {
    h.respond("issue.json");
    await plan(["init", "ABC-123"]);
    const step = {
      id: "key-is-right",
      title: "issue exists",
      check: `"$JIRA" issue get ABC-123 --lean | jq -e '.key=="ABC-123"' >/dev/null`,
    };
    writeFileSync(
      DOC(),
      readFileSync(DOC(), "utf8").replace("\n[]\n", `\n${JSON.stringify([step])}\n`),
    );
    expect(parseSteps(DOC())).toHaveLength(1);
    const run = await plan(["run", ".claude/tmp/jira/plans/ABC-123.md"]);
    expect(run.status).toBe(0);
    const ledger = JSON.parse(readFileSync(LEDGER(), "utf8"));
    expect(ledger).toMatchObject({ base_branch: "", steps: [{ status: "green" }] });
  });
});

describe("ledger contract", () => {
  // One green and one red step: both verdict branches land in the emitted doc.
  beforeEach(async () => {
    copyFileSync(join(FIXTURES, "plan-ok.md"), join(repo, "plan.md"));
    h.respond("issue.json");
    await plan(["run", "plan.md"]);
  });

  const read = () => JSON.parse(readFileSync(LEDGER(), "utf8"));

  test("contract: base_branch is exactly the empty string", () => {
    // state.ts currentDiffSha() returns null for an empty base, so a green Jira
    // card never flips amber when an unrelated code commit lands.
    expect(read().base_branch).toBe("");
  });

  test("contract: version is 1, matching the push gate's hard schema check", () => {
    expect(read().version).toBe(1);
  });

  test("contract: step status is always one of the four allowed values", () => {
    for (const step of read().steps)
      expect(["green", "red", "pending", "running"]).toContain(step.status);
  });

  test("contract: summary agrees with steps, and stale is 0", () => {
    const ledger = read();
    const count = (status: string) =>
      ledger.steps.filter((step: { status: string }) => step.status === status).length;
    expect(ledger.summary).toEqual({
      total: ledger.steps.length,
      green: count("green"),
      red: count("red"),
      pending: count("pending"),
      running: count("running"),
      stale: 0,
      fresh_green: count("green"),
      retried: 0,
    });
  });

  test("contract: the ledger filename is jira-<KEY>.json, invisible to the push gate", () => {
    expect(existsSync(LEDGER())).toBe(true);
    const branch = spawnSync("git", ["-C", repo, "rev-parse", "--abbrev-ref", "HEAD"], {
      encoding: "utf8",
    });
    const slug = branch.stdout.trim().replaceAll("/", "-");
    expect(existsSync(join(repo, `.claude/tmp/plan-ledger/${slug}.json`))).toBe(false);
  });

  test("contract: the ledger identifies its project via .branch", () => {
    expect(read().branch).toBe("jira-ABC-123");
  });
});
