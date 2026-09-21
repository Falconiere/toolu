/** Shared filesystem roots for the gate-coverage inventory CLI. */
import { join } from "node:path";

export const ROOT = join(import.meta.dir, "../../..");
export const INVENTORY =
  process.env.GATE_COVERAGE_INVENTORY ??
  join(ROOT, "tooling/fixtures/gate-coverage/inventory.json");
export const MATRIX =
  process.env.GATE_COVERAGE_MATRIX ?? join(ROOT, "docs/gate-coverage-matrix.md");
