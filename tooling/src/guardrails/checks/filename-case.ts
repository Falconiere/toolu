/**
 * filename-case — filenames follow the language's casing convention (snake_case
 * for .rs). The TypeScript stacks omit `filenameCase`: oxlint's
 * unicorn/filename-case owns kebab-case there. Omitted key, no-op.
 */
import type { FilenameRule, GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { ereToRegExp, matchGlob } from "../glob.ts";
import { findFiles, isDir } from "../walk.ts";

function report(ctx: CheckContext<GuardrailsConfig>, rule: FilenameRule, path: string): void {
  const base = path.slice(path.lastIndexOf("/") + 1);
  if (!matchGlob(rule.glob, base) || ereToRegExp(rule.regex).test(base)) return;
  ctx.report.violation(
    "filename-case",
    path,
    `filename is not ${rule.describe}`,
    "rename it — the convention is what makes a grep for a symbol land on its file",
  );
}

export function filenameCase(ctx: CheckContext<GuardrailsConfig>, mode: Mode, path: string): void {
  const { srcRoot, filenameCase: rules } = ctx.config;
  for (const rule of rules) {
    if (mode === "file") {
      // Scoped to srcRoot: build.rs and root-level configs are not source.
      if (path.startsWith(`${srcRoot}/`)) report(ctx, rule, path);
      continue;
    }
    if (!isDir(ctx.root, srcRoot)) continue;
    for (const file of findFiles(ctx.root, srcRoot)) report(ctx, rule, file);
  }
}
