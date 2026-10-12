/**
 * The one golden deviation of the native port (#428): a Rust file in a linked
 * worktree is recorded in that worktree's own gate file, the state the shared
 * Rust runner (#459) gives every quality rule, on both post-tools legs.
 */
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { OWN_WORKTREE_GATE, RS_CASES } from "../cases.ts";
import { dispatchStep, setupCase, type Leg } from "../golden-harness.ts";

const linked = RS_CASES.find((c) => c.name === OWN_WORKTREE_GATE);

for (const leg of ["native", "bun"] satisfies Leg[]) {
  test.concurrent(`a linked worktree file fails its own gate [${leg}]`, async () => {
    if (linked === undefined) throw new Error(`no case named ${OWN_WORKTREE_GATE}`);
    using sb = createSandbox({ git: true });
    await setupCase(sb, "claude", linked, { kind: "native" });
    for (const step of linked.steps) {
      const res = await dispatchStep(sb, "claude", linked, step, leg);
      expect(res.stdout).toContain("Forbidden lint suppression");
    }
    const gate = JSON.parse(readFileSync(sb.path("wt/.claude/tmp/quality-gate-status.json"), "utf8"));
    expect(gate).toMatchObject({
      status: "failing",
      entries: { [sb.path("wt/src/bad.rs")]: { source: "rust-quality-hook" } },
    });
  }, 60_000);
}
