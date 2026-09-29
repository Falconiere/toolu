/**
 * AC-3 (#256): `ledgerMain` status, path, root, `--self-test` and usage vs
 * `plan-ledger.sh` on twin real repos. It covers orphan healing, a fresh
 * running step, AC coverage (covered, uncovered, stale, `Spec: none`), an
 * absent ledger, subdirectory invocation, and model tiers. Each compares the
 * exit code, stdout, the authored stderr and the resulting ledger bytes.
 */
import { describe, expect, test } from "bun:test";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { ledgerMain } from "../ledger-commands.ts";
import {
  LEDGER,
  commit,
  planDoc,
  readText,
  step,
  twin,
  type Action,
  type Scenario,
} from "./ledger-scenario.ts";

const iso = (secondsAgo: number): string =>
  `${new Date(Date.now() - secondsAgo * 1000).toISOString().slice(0, 19)}Z`;

const SPEC = `# Spec

**Status:** Approved

## Acceptance criteria

- **AC-1:** first
- **AC-2:** second
`;

const write =
  (steps: unknown[], header = "") =>
  (sb: Sandbox): void => {
    sb.write("plan.md", planDoc(steps, header));
  };
const run = (...flags: string[]): Action => ({
  argv: (sb) => ["run", sb.path("plan.md"), ...flags],
});
const cmd = (...argv: string[]): Action => ({ argv: () => argv });
const status = cmd("status");

/** Rewrite the ledger through JSON (pretty, like the bats jq patch). */
function patchLedger(sb: Sandbox, fn: (steps: Record<string, unknown>[]) => void): void {
  const ledger = JSON.parse(readText(sb, LEDGER)) as { steps: Record<string, unknown>[] };
  fn(ledger.steps);
  sb.write(LEDGER, `${JSON.stringify(ledger, null, 2)}\n`);
}

const SCENARIOS: Record<string, Scenario> = {
  "partial ledger: next=s2, exit 1": {
    setup: write([step("s1", "true"), step("s2", "false")]),
    actions: [run(), status],
  },
  "all fresh-green: next=none, exit 0": {
    setup: write([step("s1", "true")]),
    actions: [run(), status],
  },
  "absent ledger: exit 2": { actions: [status] },
  "orphaned, fresh and started_at-less running steps": {
    setup: write([step("s1", "true"), step("s2", "true"), step("s3", "true")]),
    actions: [
      run(),
      {
        ...status,
        before: (sb) =>
          patchLedger(sb, (steps) => {
            Object.assign(steps[0] ?? {}, {
              status: "running",
              started_at: iso(600),
              activity: "stuck",
            });
            Object.assign(steps[1] ?? {}, {
              status: "running",
              started_at: iso(5),
              activity: "live",
            });
            Object.assign(steps[2] ?? {}, { status: "running", started_at: null });
          }),
      },
      { ...status, env: { PL_STUCK_THRESHOLD: "1" } },
    ],
  },
  "AC coverage: covered, uncovered, then stale": {
    setup: (sb) => {
      sb.write("spec.md", SPEC);
      write(
        [step("s1", "true", { ac_refs: ["AC-1"] }), step("s2", "true")],
        `**Status:** Approved   **Spec:** spec.md`,
      )(sb);
    },
    actions: [
      run(),
      status,
      {
        ...status,
        before: (sb) => {
          sb.write("x.ts", "x\n");
          commit(sb);
        },
      },
    ],
  },
  "Spec: none skips the coverage section": {
    setup: write(
      [step("s1", "true", { ac_refs: ["AC-1"] })],
      "**Status:** Approved   **Spec:** none",
    ),
    actions: [run(), status],
  },
  "relative plan_doc and spec from a subdirectory": {
    setup: (sb) => {
      sb.write("spec.md", SPEC);
      sb.write("sub/.keep", "");
      write([step("s1", "true", { ac_refs: ["AC-2"] })], "**Spec:** spec.md")(sb);
    },
    actions: [{ argv: () => ["run", "plan.md"] }, { ...status, cwd: "sub" }],
  },
  "model tier of the next step shows on status": {
    setup: write([step("s1", "true"), step("s2", "false", { model: "opus" })]),
    actions: [run(), status],
  },
  "status keeps retries byte-identical": {
    setup: write([step("s1", "false")]),
    actions: [run("--step", "s1"), run("--step", "s1"), status, status],
  },
  "legacy ledger recomputes": {
    setup: (sb) =>
      sb.write(LEDGER, {
        version: 1,
        branch: "feat/x",
        plan_doc: "gone.md",
        steps: [{ id: "s1", title: "t", check: "true", status: "green", diff_sha: "abc" }],
      }),
    actions: [status],
  },
  "path, root, --self-test, usage": {
    setup: (sb) => sb.write("sub/.keep", ""),
    actions: [
      { ...cmd("path"), cwd: "sub" },
      { ...cmd("root"), cwd: "sub" },
      cmd("--self-test"),
      cmd(),
      cmd("bogus"),
      cmd("run"),
      cmd("run", ""),
      { ...cmd("path"), env: { TOOLU_HOST_OVERRIDE: "codex" } },
    ],
  },
  "outside a repository": {
    noRepo: true,
    actions: [cmd("path"), cmd("root"), cmd("status"), cmd("--self-test")],
  },
  "unborn HEAD": {
    noRepo: true,
    setup: (sb) => sb.git("init", "-q", "-b", "main"),
    actions: [cmd("path"), cmd("root"), cmd("status")],
  },
};

describe("ledgerMain vs plan-ledger.sh (status and friends)", () => {
  for (const [name, scenario] of Object.entries(SCENARIOS)) {
    test.concurrent(
      name,
      async () => {
        const [bash, port] = await twin(scenario, ledgerMain);
        expect(port).toEqual(bash);
      },
      60_000,
    );
  }
});
