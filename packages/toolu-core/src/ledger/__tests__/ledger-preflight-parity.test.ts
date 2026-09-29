/**
 * AC-5 (#256): `ledgerMain(["preflight", ...])` vs `plan-ledger.sh
 * preflight`. The scenarios are the 9 `plan-ledger-preflight.bats` cases, plus
 * no-arg resolution through the ledger's `plan_doc`, orphan healing (and the
 * text-inequality rewrite of a compact ledger), subdirectory resolution, and a
 * directory given as the plan.
 */
import { describe, expect, test } from "bun:test";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { ledgerMain } from "../ledger-commands.ts";
import { LEDGER, planDoc, step, twin, type Action, type Scenario } from "./ledger-scenario.ts";

const doc = (header: string): string => planDoc([step("s1", "true")], header);
const spec = (status: string): string => `# Spec\n\n**Date:** 2026-09-29   **Status:** ${status}\n`;

function files(map: Record<string, string>) {
  return (sb: Sandbox): void => {
    for (const [rel, body] of Object.entries(map)) sb.write(rel, body);
  };
}

const preflight = (...args: string[]): Action => ({ argv: () => ["preflight", ...args] });
const plan = preflight("plan.md");

const SCENARIOS: Record<string, Scenario> = {
  "plan Draft": {
    setup: files({ "plan.md": doc("**Status:** Draft   **Spec:** none") }),
    actions: [plan],
  },
  "spec Needs changes": {
    setup: files({
      "plan.md": doc("**Status:** Approved   **Spec:** spec.md"),
      "spec.md": spec("Needs changes"),
    }),
    actions: [plan],
  },
  "both approved": {
    setup: files({
      "plan.md": doc("**Status:** Approved   **Spec:** spec.md"),
      "spec.md": spec("Approved"),
    }),
    actions: [plan],
  },
  "approval is case-insensitive": {
    setup: files({
      "plan.md": doc("**Status:** APPROVED   **Spec:** spec.md"),
      "spec.md": spec("approved"),
    }),
    actions: [plan],
  },
  "Spec none": {
    setup: files({ "plan.md": doc("**Status:** Approved   **Spec:** NONE") }),
    actions: [plan],
  },
  "no Spec line": { setup: files({ "plan.md": doc("**Status:** Approved") }), actions: [plan] },
  "declared spec missing": {
    setup: files({ "plan.md": doc("**Status:** Approved   **Spec:** docs/gone.md") }),
    actions: [plan],
  },
  "spec without a Status": {
    setup: files({
      "plan.md": doc("**Status:** Approved   **Spec:** spec.md"),
      "spec.md": "# Spec\n",
    }),
    actions: [plan],
  },
  "header-less plan": { setup: files({ "plan.md": doc("") }), actions: [plan] },
  "missing plan doc": { actions: [preflight("nope.md")] },
  "directory as the plan": { setup: files({ "dir/.keep": "" }), actions: [preflight("dir")] },
  "no arg and no ledger": { actions: [preflight()] },
  "no arg: resolves the ledger plan_doc and heals an orphan": {
    setup: (sb) => {
      files({ "plan.md": doc("**Status:** Approved   **Spec:** none") })(sb);
      const old = `${new Date(Date.now() - 900_000).toISOString().slice(0, 19)}Z`;
      sb.write(LEDGER, {
        version: 1,
        plan_doc: "plan.md",
        steps: [{ id: "s1", status: "running", started_at: old, activity: "x" }],
      });
    },
    actions: [preflight()],
  },
  "a compact ledger is rewritten even with nothing to heal": {
    setup: (sb) => {
      files({ "plan.md": doc("**Status:** Draft") })(sb);
      sb.write(LEDGER, '{"version":1,"plan_doc":"plan.md","steps":[]}');
    },
    actions: [preflight()],
  },
  "an unhealable ledger is left alone": {
    setup: (sb) => {
      files({ "plan.md": doc("**Status:** Approved") })(sb);
      sb.write(LEDGER, '{"plan_doc":"plan.md","steps":5}');
    },
    actions: [preflight(), preflight("plan.md")],
  },
  "relative plan from a subdirectory resolves against the root": {
    setup: files({
      "plan.md": doc("**Status:** Approved   **Spec:** docs/spec.md"),
      "docs/spec.md": spec("Approved"),
      "sub/.keep": "",
    }),
    actions: [{ ...plan, cwd: "sub" }],
  },
  "absolute plan path": {
    setup: files({ "plan.md": doc("**Status:** Draft") }),
    actions: [{ argv: (sb) => ["preflight", sb.path("plan.md")] }],
  },
};

describe("ledgerMain preflight vs plan-ledger.sh preflight", () => {
  for (const [name, scenario] of Object.entries(SCENARIOS)) {
    test.concurrent(name, async () => {
      const [bash, port] = await twin(scenario, ledgerMain);
      expect(port).toEqual(bash);
    });
  }
});
