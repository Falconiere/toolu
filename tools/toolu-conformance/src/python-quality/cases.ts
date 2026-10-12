/** Every python-quality golden case (#266), loaded from shared JSON. */
import { resolve } from "node:path";
import { readQualityCases } from "@toolu/conformance/harness/quality-cases";
import type { PyCase } from "./cases-types.ts";

export const PY_CASES: readonly PyCase[] = readQualityCases(
  resolve(import.meta.dir, "../../../../fixtures/quality/python.json"),
);

/**
 * The shared Rust runner (#459) records a file in a linked worktree in that
 * worktree's own gate file, where bash used the hook's project root. Output is
 * unchanged and the main project keeps no entry; `worktree-gate.test.ts`
 * checks the worktree's entry.
 */
export const OWN_WORKTREE_GATE = "flow: a file in a linked worktree is checked";
