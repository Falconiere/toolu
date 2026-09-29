/**
 * folder-readmes — every indexed folder carries its README.md: the
 * `src.requireReadme` directories, and every domain folder under a
 * `src.nested` "x/*" key. A README is never linted, so this cannot be owned by
 * a linter rule; repo scope only.
 */
import { readdirSync } from "node:fs";
import { join } from "node:path";
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { isDir, isFile } from "../walk.ts";

/** `"$dir"/*\/`: non-hidden entries that are directories (symlinks followed), sorted. */
function subdirs(root: string, dir: string): string[] {
  let names: string[];
  try {
    names = readdirSync(join(root, dir));
  } catch {
    return [];
  }
  return names
    .filter((name) => !name.startsWith(".") && isDir(root, `${dir}/${name}`))
    .toSorted()
    .map((name) => `${dir}/${name}`);
}

export function folderReadmes(ctx: CheckContext<GuardrailsConfig>, mode: Mode): void {
  const { srcRoot, requireReadme, nested } = ctx.config;
  if (mode !== "repo" || !isDir(ctx.root, srcRoot)) return;

  for (const name of requireReadme) {
    const dir = `${srcRoot}/${name}`;
    if (isDir(ctx.root, dir) && !isFile(ctx.root, `${dir}/README.md`)) {
      ctx.report.violation(
        "folder-readmes",
        dir,
        "missing README.md",
        'add one from templates/folder-README.md — it is how an agent answers "where does X live" without reading the tree',
      );
    }
  }

  for (const [key] of nested) {
    if (!key.endsWith("/*")) continue;
    const parent = `${srcRoot}/${key.slice(0, -2)}`;
    if (!isDir(ctx.root, parent)) continue;
    for (const domain of subdirs(ctx.root, parent)) {
      if (!isFile(ctx.root, `${domain}/README.md`)) {
        ctx.report.violation(
          "folder-readmes",
          domain,
          "domain folder has no README.md",
          "add one listing what belongs in this domain and what does not",
        );
      }
    }
  }
}
