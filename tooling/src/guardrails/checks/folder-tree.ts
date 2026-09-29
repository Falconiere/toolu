/**
 * folder-tree — the allowed source tree shape.
 *
 *   src.topLevel  the only directories permitted directly under srcRoot
 *                 (omitted = unconstrained; [] = nothing allowed)
 *   src.nested    a glob-keyed allowlist of subdirectories: "domains/*"
 *                 constrains every feature folder without naming any
 */
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { findDirs, isDir } from "../walk.ts";

/** The allowlist governing `parent`'s children; [] when no key matches (unconstrained). */
function allowFor(nested: GuardrailsConfig["nested"], parent: string): readonly string[] {
  for (const [key, values] of nested) {
    if (key === "") continue;
    if (key.endsWith("/*")) {
      // "domains/*" governs the children of every direct child of domains/
      const prefix = key.slice(0, -2);
      if (parent.startsWith(`${prefix}/`) && !parent.slice(prefix.length + 1).includes("/")) {
        return values;
      }
    } else if (key === parent || key === "*") {
      // A bare "*" governs every directory at any depth when nothing more specific matched.
      return values;
    }
  }
  return [];
}

function checkDir(ctx: CheckContext<GuardrailsConfig>, rel: string, shown: string): void {
  const { srcRoot, topLevel, testDir, nested } = ctx.config;
  if (rel === "" || rel === ".") return;
  const top = rel.split("/")[0] ?? rel;
  if (topLevel !== null && !topLevel.includes(top)) {
    ctx.report.violation(
      "folder-tree",
      shown,
      `"${top}" is not an allowed top-level directory under ${srcRoot}`,
      `move it under one of: ${topLevel.join(" ")}`,
    );
  }
  const slash = rel.lastIndexOf("/");
  if (slash === -1) return;
  const parent = rel.slice(0, slash);
  const leaf = rel.slice(slash + 1);
  const allow = allowFor(nested, parent);
  // The test directory is allowed inside ANY constrained directory: CORE rule 6
  // requires tests in a sibling test dir, so no allowlist may forbid it.
  if (allow.length === 0 || leaf === testDir || allow.includes(leaf)) return;
  ctx.report.violation(
    "folder-tree",
    shown,
    `"${leaf}" is not an allowed directory inside ${srcRoot}/${parent}`,
    `use one of: ${allow.join(" ")}`,
  );
}

export function folderTree(ctx: CheckContext<GuardrailsConfig>, mode: Mode, path: string): void {
  const { srcRoot } = ctx.config;
  if (!isDir(ctx.root, srcRoot)) return;

  if (mode === "file") {
    // The edited file's directory and every directory above it up to srcRoot:
    // one new file can introduce several at once.
    const slash = path.lastIndexOf("/");
    const dir = slash === -1 ? path : path.slice(0, slash);
    if (!dir.startsWith(`${srcRoot}/`)) return;
    let rel = dir.slice(srcRoot.length + 1);
    while (rel !== "" && rel !== ".") {
      checkDir(ctx, rel, `${srcRoot}/${rel}`);
      const up = rel.lastIndexOf("/");
      if (up === -1) break;
      rel = rel.slice(0, up);
    }
    return;
  }

  for (const dir of findDirs(ctx.root, srcRoot)) {
    checkDir(ctx, dir.slice(srcRoot.length + 1), dir);
  }
}
