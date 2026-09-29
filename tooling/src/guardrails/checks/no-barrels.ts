/**
 * no-barrels — no re-export layer. With no barrel to hide behind, a grep for
 * a symbol lands on its definition and an export nothing imports is genuinely
 * dead, which is what makes knip accurate. barrelExempt is per stack (console
 * exempts src/app/** where index.tsx is TanStack Router's "/" route).
 */
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { matchAny } from "../glob.ts";
import { findFiles, isDir } from "../walk.ts";

function report(ctx: CheckContext<GuardrailsConfig>, path: string): void {
  const base = path.slice(path.lastIndexOf("/") + 1);
  const name = ctx.config.barrelNames.find((candidate) => candidate === base);
  if (name === undefined || matchAny(ctx.config.barrelExempt, path)) return;
  // mod.rs gets its own remedy: Rust has no knip; the move is renaming the file
  // up a level next to its folder and declaring the submodules there.
  if (name === "mod.rs") {
    ctx.report.violation(
      "no-barrels",
      path,
      "mod.rs barrel",
      "rename it to a sibling file next to its folder (src/foo/mod.rs → src/foo.rs) and declare the submodules there",
    );
    return;
  }
  ctx.report.violation(
    "no-barrels",
    path,
    "barrel file",
    "delete it and import the concrete file — a re-export layer hides dead code from knip",
  );
}

export function noBarrels(ctx: CheckContext<GuardrailsConfig>, mode: Mode, path: string): void {
  const { srcRoot, barrelNames } = ctx.config;
  if (barrelNames.length === 0) return;
  if (mode === "file") {
    // Scoped to srcRoot: unscoped, Expo's root-level app/index.tsx routes read as barrels.
    if (path.startsWith(`${srcRoot}/`)) report(ctx, path);
    return;
  }
  if (!isDir(ctx.root, srcRoot)) return;
  for (const file of findFiles(ctx.root, srcRoot)) report(ctx, file);
}
