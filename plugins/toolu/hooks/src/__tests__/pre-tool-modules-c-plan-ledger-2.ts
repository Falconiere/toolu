/**
 * plan-ledger cases (#262), part 2: the `planLedger.blockOnUncoveredAcs` cases
 * of the deleted plan-ledger-acblock.bats, the `ac_coverage` telemetry test of
 * telemetry-sites.bats and the plan-ledger parity test of diff-sha.bats. The
 * bats suite ran the real checker (`plan-ledger.sh run <plan> --verify`) to
 * write its ledgers; these hand-author the ledger it writes: steps stamped with
 * the current diff sha, `ac_refs`, and `verified_sha`. Plan and spec docs sit
 * in the project and are named by relative path.
 */
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { group, writeIn, type GateCase } from "./pre-tool-modules-c-cases.ts";
import { cfg, put, seed, step } from "./pre-tool-modules-c-plan-ledger-kit.ts";

const SPEC =
  "# Fixture Spec\n\n## Acceptance criteria\n\n- **AC-1:** first criterion.\n- **AC-2:** second criterion.\n";

function planDoc(spec: string | null): string {
  const specField = spec === null ? "" : `**Spec:** ${spec}\n\n`;
  return `# Fixture Plan\n\n${specField}## Steps (machine-readable)\n\n\`\`\`json\n[]\n\`\`\`\n`;
}

/** A verified, fresh-green ledger whose steps reference `refs` (one step per entry). */
function acSetup(spec: boolean, refs: string[][]) {
  return (sb: Sandbox) => {
    const sha = seed(sb);
    writeIn(sb.project, "spec.md", SPEC);
    writeIn(sb.project, "plan.md", planDoc(spec ? "spec.md" : null));
    const steps = refs.map((ac_refs, i) =>
      step(`s${i + 1}`, `covers ${ac_refs.join(",")}`, "green", sha, { ac_refs }),
    );
    put(sb, steps, { plan_doc: "plan.md", verified_sha: sha });
  };
}

const block = { planLedger: { blockOnUncoveredAcs: true } };
const acs = group({ command: "git push" });

export const PLAN_LEDGER_AC_CASES: GateCase[] = [
  acs({
    name: "plan-ledger: blockOnUncoveredAcs=true denies push naming the uncovered AC id",
    config: cfg(block, { planLedger: { mode: "block" } }),
    setup: acSetup(true, [["AC-1"]]),
    expect: "deny",
    has: ["AC-2"],
    lacks: ["AC-1"],
  }),
  acs({
    name: "plan-ledger: blockOnUncoveredAcs default false allows push, ac_coverage still recorded",
    config: cfg({}, {}),
    setup: acSetup(true, [["AC-1"]]),
    expect: "silent",
  }),
  acs({
    name: "plan-ledger: blockOnUncoveredAcs=true with every AC covered allows the push",
    config: cfg(block, { planLedger: { mode: "block" } }),
    setup: acSetup(true, [["AC-1"], ["AC-2"]]),
    expect: "silent",
  }),
  acs({
    name: "plan-ledger: spec-less plan never blocks even with blockOnUncoveredAcs=true",
    config: cfg(block, { planLedger: { mode: "block" } }),
    setup: acSetup(false, [[]]),
    expect: "silent",
  }),
  acs({
    name: "plan-ledger: blockOnUncoveredAcs=true advises rather than denies under the shipped preset",
    config: cfg(block, {}),
    setup: acSetup(true, [["AC-1"]]),
    expect: "advisory",
    has: ["AC-2"],
  }),
  acs({
    name: "plan-ledger: gates.planLedger.mode off silences the ledger gate entirely",
    config: cfg(block, { planLedger: { mode: "off" } }),
    setup: acSetup(true, [["AC-1"]]),
    expect: "silent",
  }),
  acs({
    // The ledger predates `verified_sha` (the telemetry-sites fixture); the
    // covered/uncovered counts are recorded as a telemetry line, not asserted.
    name: "plan-ledger: ac_coverage records covered/uncovered counts from a plan+spec+ledger fixture",
    command: "git push origin feat/example",
    config: cfg({}, {}),
    setup: (sb) => {
      const sha = seed(sb);
      writeIn(sb.project, "spec.md", SPEC);
      writeIn(sb.project, "plan.md", planDoc("spec.md"));
      put(sb, [step("s1", "covers AC-1", "green", sha, { ac_refs: ["AC-1"] })], {
        plan_doc: "plan.md",
      });
    },
    expect: "silent",
  }),
  acs({
    name: "plan-ledger: parity (plan-ledger.sh gate): ledger stamped with raw-formula sha is fresh-green",
    command: "git push origin feat/example",
    config: cfg({}, {}),
    setup: (sb) => {
      const sha = seed(sb);
      put(sb, [step("s1", "first", "green", sha)], {
        plan_doc: "docs/plan.md",
        updated_at: "2026-07-30T00:00:00Z",
      });
    },
    expect: "silent",
  }),
];
