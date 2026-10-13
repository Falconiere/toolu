/**
 * The one golden deviation of the native port (#427): a Python file in a
 * linked worktree is recorded in that worktree's own gate file, the state the
 * shared Rust runner (#459) gives every quality rule, on both post-tools legs.
 */
import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { OWN_WORKTREE_GATE, PY_CASES } from "../cases.ts";
import { dispatchStep, setupCase, type Leg } from "../golden-harness.ts";

const linked = PY_CASES.find((c) => c.name === OWN_WORKTREE_GATE);

for (const leg of ["native", "bun"] satisfies Leg[]) {
  test.concurrent(`a linked worktree file fails its own gate [${leg}]`, async () => {
    if (linked === undefined) throw new Error(`no case named ${OWN_WORKTREE_GATE}`);
    using sb = createSandbox({ git: true });
    await setupCase(sb, "claude", linked, { kind: "native" });
    for (const step of linked.steps) {
      const res = await dispatchStep(sb, "claude", linked, step, leg);
      expect(res.stdout).toContain(`Forbidden suppression in ${sb.path("wt/bad.py")} — `);
    }
    const gate = JSON.parse(readFileSync(sb.path("wt/.claude/tmp/quality-gate-status.json"), "utf8"));
    expect(gate.status).toBe("failing");
    expect(Object.keys(gate.entries)).toEqual([sb.path("wt/bad.py")]);
    expect(gate.entries[sb.path("wt/bad.py")].source).toBe("python-quality-hook");
    expect(existsSync(sb.path(".claude/tmp/quality-gate-status.json"))).toBe(false);
  }, 60_000);
}
