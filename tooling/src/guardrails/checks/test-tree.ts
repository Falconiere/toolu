/**
 * test-tree — no centralized test directory. The companion to colocated-tests:
 * that one catches a stray test FILE, this one the centralized DIRECTORY it
 * would land in. A directory is never linted, so no linter rule can own this.
 */
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { isDir } from "../walk.ts";

export function testTree(ctx: CheckContext<GuardrailsConfig>, mode: Mode): void {
  if (mode !== "repo") return;
  const { srcRoot, testDir } = ctx.config;
  for (const centralized of [`${srcRoot}/__tests__`, `${srcRoot}/tests`, "tests", "test"]) {
    if (centralized === `${srcRoot}/${testDir}`) continue;
    // Rust's crate-root tests/ is the documented integration-test surface.
    if (centralized === "tests" && testDir === "tests") continue;
    if (isDir(ctx.root, centralized)) {
      ctx.report.violation(
        "test-tree",
        centralized,
        "centralized test directory",
        `colocate each test in a sibling ${testDir}/ instead`,
      );
    }
  }
}
