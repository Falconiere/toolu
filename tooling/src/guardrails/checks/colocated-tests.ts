/**
 * colocated-tests — a test FILE lives beside the code it exercises, in a
 * sibling __tests__/ (TS) or tests/ (Rust). testGlob is load-bearing: marketing
 * matches *.test.tsx too, so a misplaced component test cannot slip by.
 * The centralized DIRECTORY half is the separate test-tree check.
 */
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { matchAny } from "../glob.ts";
import { findFiles, isDir } from "../walk.ts";

function report(ctx: CheckContext<GuardrailsConfig>, path: string): void {
  const { testDir, testGlobs } = ctx.config;
  const base = path.slice(path.lastIndexOf("/") + 1);
  if (!matchAny(testGlobs, base) || path.includes(`/${testDir}/`)) return;
  ctx.report.violation(
    "colocated-tests",
    path,
    `test file outside a ${testDir}/ directory`,
    `move it to a sibling ${testDir}/ beside the file it covers`,
  );
}

export function colocatedTests(
  ctx: CheckContext<GuardrailsConfig>,
  mode: Mode,
  path: string,
): void {
  const { srcRoot } = ctx.config;
  if (mode === "file") {
    // Scoped to srcRoot like the repo walk: root-level files are never looked at.
    if (path.startsWith(`${srcRoot}/`)) report(ctx, path);
    return;
  }
  if (!isDir(ctx.root, srcRoot)) return;
  for (const file of findFiles(ctx.root, srcRoot)) report(ctx, file);
}
