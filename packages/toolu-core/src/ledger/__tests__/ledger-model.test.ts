/**
 * The pure ledger model (#256; AC-3, AC-4) against the bash functions in
 * `plan-ledger.sh` and `plan-ledger-preflight.sh`. The inputs are
 * well-formed, legacy and malformed ledgers. Each function must produce the
 * same bytes as its bash original, or fail where bash fails.
 */
import { describe, expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { toJqJson } from "../../state/state-io.ts";
import { JqError, type Json, type JsonObject } from "../ledger-jq.ts";
import {
  acCoverage,
  buildStepEntry,
  entriesById,
  healOrphans,
  orphanCutoff,
  recompute,
  summaryLine,
} from "../ledger-model.ts";
import { bashFn, type Outcome } from "./ledger-parity-helpers.ts";

const CUR = "c".repeat(40);
const OLD = "o".repeat(40);
const ago = (seconds: number): string =>
  `${new Date(Date.now() - seconds * 1000).toISOString().slice(0, 19)}Z`;

const step = (id: Json, extra: JsonObject = {}): JsonObject => ({
  id,
  title: `t-${String(id)}`,
  check: "true",
  status: "pending",
  ...extra,
});

const LEDGERS: Record<string, Json> = {
  mixed: {
    version: 1,
    summary: {},
    next: null,
    steps: [
      step("s1", { status: "green", diff_sha: CUR, ac_refs: ["AC-1"] }),
      step("s2", { status: "green", diff_sha: OLD, model: "haiku", ac_refs: ["AC-2", "AC-1"] }),
      step("s3", { status: "red", diff_sha: CUR, retries: [{ attempt: 1 }] }),
      step("s4", { status: "running", started_at: ago(600), activity: "x" }),
      step("s5", { status: "running", started_at: ago(5) }),
      step("s6", { status: "running", started_at: "" }),
      step("s7", { status: "running", started_at: 12 }),
      step("s8", { status: "running", started_at: ["x"] }),
    ],
  },
  "all fresh": { steps: [step("s1", { status: "green", diff_sha: CUR })] },
  scoped: {
    steps: [
      step("a", { status: "green", diff_sha: OLD, scope_sha: "sa" }),
      step("b", { status: "green", diff_sha: OLD, scope_sha: "zz" }),
      step("c", { status: "green", diff_sha: CUR }),
    ],
  },
  legacy: {
    version: 1,
    steps: [{ id: "s1", title: "t", check: "c", status: "green", diff_sha: CUR }],
  },
  "empty steps": { version: 1, steps: [] },
  "null step": { steps: [null] },
  "no steps": {},
  array: [],
  "steps number": { steps: 5 },
  "steps [1]": { steps: [1] },
  "steps object": { steps: { a: step("a", { status: "green", diff_sha: CUR }) } },
  "numeric id green": { steps: [step(7, { status: "green", diff_sha: CUR })] },
  "numeric next": { next: 3, summary: { fresh_green: 0, total: 1 }, steps: [step(3)] },
  "false model on next": {
    next: "s1",
    summary: { total: 1 },
    steps: [step("s1", { model: false })],
  },
  "retries string": { steps: [step("s1", { retries: "abc" })] },
  "retries true": { steps: [step("s1", { retries: true })] },
  "status number": { steps: [step("s1", { status: 3 })] },
  "ac_refs string": {
    steps: [step("s1", { status: "green", diff_sha: CUR, ac_refs: "AC-1 AC-2" })],
  },
  "ac_refs object": { steps: [step("s1", { ac_refs: { "AC-1": [0] } })] },
  "null id": { steps: [step(null), step("s2")] },
  "duplicate ids": { steps: [step("s1", { status: "red" }), step("s1", { status: "green" })] },
  "DEL and unicode": { steps: [step("é\u007f", { status: "green", diff_sha: CUR })] },
};

type TsOutcome = Outcome;

function tsOutcome(fn: () => string): TsOutcome {
  try {
    return { code: 0, stdout: fn(), stderr: [] };
  } catch (error) {
    if (error instanceof JqError) return { code: 1, stdout: "", stderr: [] };
    throw error;
  }
}

/** bash: non-zero exit collapses to 1 (jq's code is not part of the contract). */
async function bashCall(fn: string, args: string[], env: Record<string, string> = {}) {
  using sb = createSandbox();
  const res = await bashFn("plan-ledger.sh", fn, args, { cwd: sb.project, env });
  return {
    code: res.exitCode === 0 ? 0 : 1,
    stdout: res.exitCode === 0 ? res.stdout : "",
    stderr: [],
  };
}

const pretty = (v: Json): string => `${toJqJson(v, true)}\n`;

describe("recompute vs pl_recompute", () => {
  const scope = { a: "sa", b: "sb" };
  for (const [name, ledger] of Object.entries(LEDGERS)) {
    for (const verify of [false, true]) {
      test.concurrent(`${name}${verify ? " --verify" : ""}`, async () => {
        const args = [JSON.stringify(ledger), CUR, JSON.stringify(scope), verify ? "1" : ""];
        const bash = await bashCall("pl_recompute", args);
        expect(tsOutcome(() => pretty(recompute(ledger, CUR, scope, verify)))).toEqual(bash);
      });
    }
  }
});

describe("summaryLine vs pl_summary_line (after recompute)", () => {
  for (const [name, ledger] of Object.entries(LEDGERS)) {
    test.concurrent(name, async () => {
      let input: Json = ledger;
      try {
        input = recompute(ledger, CUR);
      } catch {
        // Keep the raw ledger: the summary line must fail or pass on it just like bash.
      }
      const bash = await bashCall("pl_summary_line", [JSON.stringify(input), "feat_x"]);
      expect(tsOutcome(() => `${summaryLine(input, "feat_x")}\n`)).toEqual(bash);
    });
  }
});

describe("healOrphans vs pl_heal_orphans", () => {
  for (const [name, ledger] of Object.entries(LEDGERS)) {
    test.concurrent(name, async () => {
      const bash = await bashCall("pl_heal_orphans", [JSON.stringify(ledger)], {
        PL_STUCK_THRESHOLD: "300",
      });
      const cutoff = orphanCutoff(new Date(), 300);
      expect(tsOutcome(() => pretty(healOrphans(ledger, cutoff)))).toEqual(bash);
    });
  }
});

describe("entriesById vs jq from_entries", () => {
  for (const [name, ledger] of Object.entries(LEDGERS)) {
    test.concurrent(name, async () => {
      const res = await run(["jq", "-c", "[.steps[] | {key: .id, value: .}] | from_entries"], {
        stdin: JSON.stringify(ledger),
      });
      const bash = res.exitCode === 0 ? res.stdout : "";
      expect(
        tsOutcome(() => `${toJqJson(Object.fromEntries(entriesById(ledger)), false)}\n`).stdout,
      ).toBe(bash);
    });
  }
});

const PLAN_STEPS = [
  step("s1", { ac_refs: ["AC-1"], depends_on: ["s0"], input: "fixture", model: "sonnet" }),
  step("s2"),
];

const PRIORS: Record<string, Json> = {
  none: null,
  green: step("s1", { status: "green", exit_code: 0, retries: [] }),
  red: step("s1", {
    status: "red",
    exit_code: 1,
    diff_sha: OLD,
    evidence_tail: "boom",
    last_run: "T0",
  }),
  "red with retries": step("s1", {
    status: "red",
    exit_code: 2,
    retries: [{ attempt: 1, exit_code: 1 }],
    last_run: "T1",
  }),
  "legacy red": { id: "s1", status: "red" },
  "red, retries string": step("s1", { status: "red", retries: "x" }),
  "prior number": 5,
};

describe("buildStepEntry vs pl_build_step_entry", () => {
  for (const [name, prior] of Object.entries(PRIORS)) {
    test.concurrent(name, async () => {
      const prev = prior === null ? "" : JSON.stringify(prior);
      const evidence = JSON.stringify("last line\n");
      const args = [JSON.stringify(PLAN_STEPS), "s1", "green", "0", CUR, evidence, "", "", prev];
      const bash = await bashCall("pl_build_step_entry", args);
      const now = bash.code === 0 ? (JSON.parse(bash.stdout) as { last_run: string }).last_run : "";
      const got = tsOutcome(() =>
        pretty(
          buildStepEntry(PLAN_STEPS[0] ?? null, prior, {
            status: "green",
            exitCode: 0,
            sha: CUR,
            evidence: "last line\n",
            now,
          }),
        ),
      );
      expect(got).toEqual(bash);
    });
  }
});

describe("acCoverage vs pl_ac_coverage_lines", () => {
  const SPEC = "## Acceptance criteria\n\n- **AC-1:** one\n- **AC-2:** two\n- **AC-3:** three\n";
  for (const [name, ledger] of Object.entries(LEDGERS)) {
    test.concurrent(name, async () => {
      using sb = createSandbox();
      const spec = sb.write("spec.md", SPEC);
      const res = await bashFn(
        "plan-ledger.sh",
        "pl_ac_coverage_lines",
        [JSON.stringify(ledger), CUR, spec],
        {
          cwd: sb.project,
        },
      );
      const got = acCoverage(ledger, CUR, spec);
      expect({ code: 0, stdout: got.stdout, stderr: got.stderr }).toEqual({
        code: res.exitCode,
        stdout: res.stdout,
        stderr: res.stderr.split("\n").filter((l) => l.startsWith("plan-ledger:")),
      });
    });
  }
  test.concurrent("spec none / missing prints nothing", async () => {
    for (const spec of ["", "none", "NONE", "/nonexistent/spec.md"]) {
      expect(acCoverage(LEDGERS.mixed ?? null, CUR, spec)).toEqual({ stdout: "", stderr: [] });
    }
  });
});
