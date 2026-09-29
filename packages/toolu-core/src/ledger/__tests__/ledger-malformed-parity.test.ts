/**
 * AC-4 (#256): malformed, legacy and odd ledgers across every command that
 * reads one: `run`, `run --step`, `status`, no-arg `preflight` and verdict.
 * Before each command the ledger is rewritten to the malformed bytes. bash
 * and TypeScript must then agree on the exit code, stdout, the authored
 * stderr lines (the user-facing messages), and the ledger bytes left behind.
 */
import { describe, expect, test } from "bun:test";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { ledgerMain } from "../ledger-commands.ts";
import type { LedgerOptions } from "../ledger-io.ts";
import { verdictMain } from "../verdict.ts";
import { LEDGER, planDoc, step, twin, type Action } from "./ledger-scenario.ts";

const PLAN = planDoc(
  [step("s1", "true", { ac_refs: ["AC-1"] }), step("s2", "true")],
  "**Status:** Approved   **Spec:** spec.md",
);
const SPEC = "**Status:** Approved\n\n## Acceptance criteria\n\n- **AC-1:** a\n";

const entry = (id: unknown, extra: Record<string, unknown> = {}) => ({
  id,
  title: "t",
  check: "true",
  status: "green",
  diff_sha: "x",
  ...extra,
});
const doc = (steps: unknown, extra: Record<string, unknown> = {}) =>
  JSON.stringify({ version: 1, plan_doc: "plan.md", summary: { total: 1 }, steps, ...extra });

const FILE = "<ROOT>/project/.claude/tmp/plan-ledger/feat_x.json";

/** Expected authored stderr per command (run, run --step, status, preflight, verdict) for a few anchors. */
const ANCHORS: Record<string, string[][]> = {
  garbage: [
    [`plan-ledger: corrupt prior ledger at ${FILE}`],
    [`plan-ledger: corrupt prior ledger at ${FILE}`],
    [`plan-ledger: no ledger at ${FILE}`],
    ["preflight: no plan doc given and no ledger plan_doc to resolve"],
    [],
  ],
  "steps number": [
    [`plan-ledger: corrupt prior ledger at ${FILE}`],
    [`plan-ledger: corrupt prior ledger at ${FILE}`],
    ["plan-ledger: failed to heal orphaned running steps"],
    [],
    [],
  ],
};

/** name → the ledger file's exact bytes. */
const LEDGERS: Record<string, string> = {
  empty: "",
  "whitespace only": "  \n",
  "json null": "null",
  "json false": "false",
  garbage: "{ nope",
  "top-level array": "[]",
  "empty object": "{}",
  "steps number": doc(5),
  "steps string": doc("ab"),
  "steps object": doc({ s1: entry("s1") }),
  "steps [1]": doc([1]),
  "steps [null]": doc([null]),
  "numeric id": doc([entry(7), entry("s2")]),
  "null id": doc([entry(null)]),
  "duplicate ids": doc([entry("s1", { status: "red" }), entry("s1")]),
  "missing summary": JSON.stringify({ version: 1, plan_doc: "plan.md", steps: [entry("s1")] }),
  "version 2": doc([entry("s1")], { version: 2 }),
  "string version": doc([entry("s1")], { version: "1" }),
  "retries string": doc([entry("s1", { status: "red", retries: "x" })]),
  "status number": doc([entry("s1", { status: 3 })]),
  "running, numeric started_at": doc([entry("s1", { status: "running", started_at: 5 })]),
  "ac_refs string": doc([entry("s1", { ac_refs: "AC-1 AC-2" })]),
  "ac_refs number": doc([entry("s1", { ac_refs: 5 })]),
  "plan_doc number": doc([entry("s1")], { plan_doc: 5 }),
  "verified_sha number": doc([entry("s1")], { verified_sha: 12 }),
  "model on a numeric next": doc([entry(3, { status: "red", model: "haiku" })]),
  compact: '{"version":1,"plan_doc":"plan.md","steps":[]}',
  "DEL and unicode": doc([entry("é\u007f")]),
};

const restore =
  (bytes: string) =>
  (sb: Sandbox): void => {
    sb.write(LEDGER, bytes);
  };

function actions(bytes: string): Action[] {
  const before = restore(bytes);
  return [
    { argv: (sb) => ["run", sb.path("plan.md")], before },
    { argv: (sb) => ["run", sb.path("plan.md"), "--step", "s1"], before },
    { argv: () => ["status"], before },
    { argv: () => ["preflight"], before },
    {
      argv: () => ["json"],
      cli: "verdict.sh",
      before,
      env: (sb) => ({ TOOLU_CONFIG_DIR: sb.home }),
    },
  ];
}

/** One TypeScript entry for both CLIs: verdict takes its mode, the ledger CLI the rest. */
function tsCli(argv: string[], options: LedgerOptions) {
  return argv[0] === "json" ? verdictMain(argv, options) : ledgerMain(argv, options);
}

describe("malformed ledgers: same messages, codes and bytes as bash", () => {
  for (const [name, bytes] of Object.entries(LEDGERS)) {
    test.concurrent(
      name,
      async () => {
        const scenario = {
          setup: (sb: Sandbox) => {
            sb.write("plan.md", PLAN);
            sb.write("spec.md", SPEC);
          },
          actions: actions(bytes),
        };
        const [bash, port] = await twin(scenario, tsCli);
        expect(port).toEqual(bash);
        const anchor = ANCHORS[name];
        if (anchor !== undefined) expect(bash.map((snap) => snap.stderr)).toEqual(anchor);
      },
      60_000,
    );
  }
});
