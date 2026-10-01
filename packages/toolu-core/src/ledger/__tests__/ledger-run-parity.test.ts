/**
 * AC-2 (#256): `ledgerRun` vs `plan-ledger.sh run` on twin real repos. The
 * scenarios are the run cases from `plan-ledger-cli.bats` and
 * `plan-ledger-model.bats`: full, `--step`, `--activity`, `--force`,
 * `--verify`, scoped `paths`, retries, legacy ledgers and model tiers, plus
 * argument errors and edge cases. Each compares the exit code, stdout, the
 * authored stderr (progress included), the ledger bytes and the telemetry.
 */
import { describe, expect, test } from "bun:test";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { ledgerRun } from "../ledger-run.ts";
import {
  LEDGER,
  TELEMETRY,
  commit,
  planDoc,
  step,
  twin,
  type Action,
  type Scenario,
  type Snapshot,
} from "./ledger-scenario.ts";

const ts = (argv: string[], options: Parameters<typeof ledgerRun>[2]) =>
  ledgerRun(argv[1] ?? "", argv.slice(2), options);

const plan = (sb: Sandbox): string => sb.path("plan.md");
const writePlan =
  (steps: unknown[], header = "") =>
  (sb: Sandbox): void => {
    sb.write("plan.md", planDoc(steps, header));
  };
const runPlan = (...flags: string[]): Action => ({ argv: (sb) => ["run", plan(sb), ...flags] });
const edit = (rel: string, body: string) => (sb: Sandbox) => {
  sb.write(rel, body);
  commit(sb);
};

const twoSteps = (c1: string, c2: string) => [step("s1", c1), step("s2", c2)];

const SCENARIOS: Record<string, Scenario> = {
  "true/false: s1 green, s2 red, exit 1": {
    setup: writePlan(twoSteps("true", "false")),
    actions: [runPlan()],
  },
  "--step flips only s2": {
    setup: writePlan(twoSteps("true", "false")),
    actions: [
      runPlan(),
      { ...runPlan("--step", "s2"), before: writePlan(twoSteps("true", "true")) },
    ],
  },
  "red evidence carries a stderr marker": {
    setup: writePlan(twoSteps("true", "echo MARKER-XYZ >&2; false")),
    actions: [runPlan()],
  },
  "no steps block: exit 2, nothing written": {
    setup: (sb) => sb.write("plan.md", "# nothing\n"),
    actions: [runPlan()],
  },
  "missing doc": { actions: [{ argv: (sb) => ["run", sb.path("absent.md")] }] },
  "staleness: a committed change re-runs green steps": {
    setup: writePlan(twoSteps("true", "true")),
    actions: [runPlan(), { ...runPlan(), before: edit("src/a.ts", "x\n") }],
  },
  "running pre-write is observable to the check": {
    setup: writePlan(twoSteps("true", "true")),
    files: ["snap.json"],
    actions: [
      runPlan(),
      {
        ...runPlan("--step", "s1", "--activity", "checking s1"),
        before: writePlan(twoSteps(`cp .claude/tmp/plan-ledger/feat_x.json snap.json`, "true")),
      },
    ],
  },
  "--activity without --step": {
    setup: writePlan(twoSteps("true", "true")),
    actions: [runPlan("--activity", "x")],
  },
  "--step without an id": {
    setup: writePlan(twoSteps("true", "true")),
    actions: [runPlan("--step")],
  },
  "--activity with an empty label": {
    setup: writePlan(twoSteps("true", "true")),
    actions: [runPlan("--step", "s1", "--activity", "")],
  },
  "unknown flag": { setup: writePlan(twoSteps("true", "true")), actions: [runPlan("--nope")] },
  "--step naming an unknown id": {
    setup: writePlan(twoSteps("true", "false")),
    actions: [runPlan(), runPlan("--step", "zz")],
  },
  "authored fields and retries: red, red, green": {
    setup: writePlan([
      step("s1", "false", { ac_refs: ["AC-1"], depends_on: [], input: "fixture" }),
      step("s2", "true", { ac_refs: ["AC-2"], depends_on: ["s1"] }),
    ]),
    actions: [
      runPlan("--step", "s1"),
      runPlan("--step", "s1"),
      {
        ...runPlan("--step", "s1"),
        before: writePlan([
          step("s1", "true", { ac_refs: ["AC-1"] }),
          step("s2", "true", { ac_refs: ["AC-3"] }),
        ]),
      },
    ],
  },
  "full re-run after a fix keeps the prior red in retries": {
    setup: writePlan(twoSteps("true", "false")),
    actions: [runPlan(), { ...runPlan(), before: writePlan(twoSteps("true", "true")) }],
  },
  "corrupt prior ledger: exit 2, file untouched": {
    setup: (sb) => {
      writePlan(twoSteps("true", "true"))(sb);
      sb.write(LEDGER, "{ not json");
    },
    actions: [runPlan(), runPlan("--step", "s1")],
  },
  "legacy v1 ledger is backfilled": {
    setup: (sb) => {
      writePlan(twoSteps("true", "true"))(sb);
      sb.write(LEDGER, {
        version: 1,
        branch: "feat/x",
        steps: [{ id: "s1", title: "old", check: "true", status: "red", exit_code: 1 }],
      });
    },
    actions: [runPlan("--step", "s2"), runPlan()],
  },
  "skip fresh, --force, explicit --step, changed diff": {
    setup: writePlan(twoSteps("true", "false")),
    actions: [
      runPlan(),
      runPlan(),
      runPlan("--force"),
      runPlan("--step", "s1"),
      { ...runPlan(), before: edit("src/b.ts", "y\n") },
    ],
  },
  "a check reading stdin does not block": {
    setup: writePlan(twoSteps("cat", "true")),
    actions: [runPlan()],
  },
  "a check that exits 124 by itself gets the timeout line": {
    setup: writePlan(twoSteps("echo partial; exit 124", "true")),
    actions: [runPlan()],
  },
  "an overrunning check is killed and marked red": {
    setup: writePlan(twoSteps("echo started; sleep 20", "true")),
    actions: [{ ...runPlan(), env: { PLAN_LEDGER_STEP_TIMEOUT: "1" } }],
  },
  "PLAN_LEDGER_STEP_TIMEOUT=0 disables the bound": {
    setup: writePlan(twoSteps("sleep 1; true", "true")),
    actions: [{ ...runPlan(), env: { PLAN_LEDGER_STEP_TIMEOUT: "0" } }],
  },
  "long, NUL and non-UTF-8 output": {
    setup: writePlan(
      twoSteps(
        "for i in $(seq 1 30); do echo line $i; done; printf 'a\\0b\\377\\n'; false",
        "true",
      ),
    ),
    actions: [runPlan()],
  },
  "scope: outside change keeps a scoped step fresh, inside re-runs, unscoped re-runs": {
    setup: (sb) => {
      sb.write("src/a.ts", "a\n");
      commit(sb);
      writePlan([step("scoped", "true", { paths: ["src/a.ts"] }), step("plain", "true")])(sb);
    },
    actions: [
      runPlan(),
      { ...runPlan(), before: edit("other.txt", "o\n") },
      { ...runPlan(), before: edit("src/a.ts", "a2\n") },
      runPlan("--verify"),
      { ...runPlan("--step", "plain"), before: edit("other.txt", "o2\n") },
      runPlan("--verify"),
    ],
  },
  "scope: a red step leaves verified_sha unset under --verify": {
    setup: writePlan([step("s1", "false", { paths: ["feature.txt"] })]),
    actions: [runPlan("--verify")],
  },
  "scope: editing paths invalidates the green": {
    setup: writePlan([step("s1", "true", { paths: ["feature.txt"] })]),
    actions: [
      runPlan(),
      {
        ...runPlan(),
        before: writePlan([step("s1", "true", { paths: ["feature.txt", "base.txt"] })]),
      },
    ],
  },
  "scope: leading-dash path, no-match path, unhashable path, empty paths": {
    setup: (sb) => {
      sb.write("-dash.txt", "x\n");
      commit(sb);
      writePlan([
        step("dash", "true", { paths: ["-dash.txt"] }),
        step("none", "true", { paths: ["nothing/here"] }),
        step("bad", "true", { paths: [":(nonsense)x"] }),
        step("empty", "true", { paths: [] }),
      ])(sb);
    },
    actions: [runPlan(), runPlan()],
  },
  "scope: malformed paths is a parse error": {
    setup: writePlan([step("s1", "true", { paths: "src" })]),
    actions: [runPlan()],
  },
  "verify: a prior verified_sha survives a later scoped run": {
    setup: writePlan([step("s1", "true", { paths: ["feature.txt"] }), step("s2", "true")]),
    actions: [
      runPlan("--verify"),
      { ...runPlan(), before: edit("other.txt", "z\n") },
      runPlan("--step", "s2"),
    ],
  },
  "model tier: suffix on the summary line, re-derived after an edit": {
    setup: writePlan([
      step("s1", "true", { model: "haiku" }),
      step("s2", "false", { model: "opus" }),
    ]),
    actions: [
      runPlan(),
      {
        ...runPlan(),
        before: writePlan([
          step("s1", "true", { model: "haiku" }),
          step("s2", "false", { model: "sonnet" }),
        ]),
      },
      { ...runPlan(), before: writePlan([step("s1", "true"), step("s2", "true")]) },
    ],
  },
  "duplicate step ids": {
    setup: writePlan([step("s1", "true"), step("s1", "echo dup"), step("s2", "true")]),
    actions: [runPlan(), runPlan("--step", "s2")],
  },
  "relative doc path from a subdirectory": {
    setup: (sb) => {
      writePlan(twoSteps("pwd", "true"))(sb);
      sb.write("sub/.keep", "");
    },
    actions: [{ argv: () => ["run", "../plan.md"], cwd: "sub" }],
  },
  "not a git repository": {
    noRepo: true,
    setup: writePlan(twoSteps("true", "true")),
    actions: [runPlan()],
  },
  "bad PUSH_REVIEW_BASE": {
    setup: writePlan(twoSteps("true", "true")),
    actions: [{ ...runPlan(), env: { PUSH_REVIEW_BASE: "no-such-ref" } }],
  },
  "ids with slash and dot": {
    setup: writePlan([step("a/b", "true"), step("c.d", "false")]),
    actions: [runPlan()],
  },
  "a check that deletes the ledger dir": {
    setup: writePlan(twoSteps("true", "rm -rf .claude/tmp/plan-ledger; true")),
    actions: [runPlan()],
  },
};

describe("ledgerRun vs plan-ledger.sh run", () => {
  // bash bounds a check only when a `timeout` binary is on PATH; without one the
  // overrun twin would diverge by design (D1), so it runs where bash can bound too.
  const noTimeout = Bun.which("timeout") === null;
  for (const [name, scenario] of Object.entries(SCENARIOS)) {
    // This case measures a one-second wall-clock timeout. Running it beside
    // dozens of subprocess-heavy twins can time out its later `true` check.
    const runs =
      noTimeout && name === "an overrunning check is killed and marked red"
        ? test.skip
        : name === "an overrunning check is killed and marked red"
          ? test.serial
          : test.concurrent;
    runs(
      name,
      async () => {
        const [bash, port] = await twin(scenario, ts);
        if (name === "duplicate step ids") pinDuplicateTelemetry(bash, port);
        expect(port).toEqual(bash);
      },
      60_000,
    );
  }
});

/**
 * D5: for a duplicated id, bash's attempt count is two JSON values, so its
 * extras are malformed and it logs a field-less `step_run` line. The closed
 * telemetry schema cannot express that line, so the port writes none. Every
 * other byte still matches.
 */
function pinDuplicateTelemetry(bash: Snapshot[], port: Snapshot[]): void {
  for (const snap of bash) {
    expect(snap.files[TELEMETRY]).toBe('{"v":1,"t":"<T>","branch":"feat/x","event":"step_run"}\n');
    snap.files[TELEMETRY] = null;
  }
  for (const snap of port) expect(snap.files[TELEMETRY]).toBeNull();
}
