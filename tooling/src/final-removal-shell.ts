/** The exact native compatibility shims permitted by the final-removal gate. */
import { spawnSync } from "node:child_process";
import { existsSync, lstatSync, readFileSync } from "node:fs";
import { join } from "node:path";

const SHIMS = new Map([
  ["plugins/jev/scripts/jev.sh", '#!/bin/sh\nexec toolu jev "$@"\n'],
  [
    "plugins/toolu-review/scripts/write-state.sh",
    '#!/bin/sh\nexec toolu review write-state "$@"\n',
  ],
]);

/** The root installer and two exact native shims; all other shell files fail. */
export const SHELL_KEEP = new Set(["install.sh", ...SHIMS.keys()]);

/** Problems in real tracked shell files and the installed shim sources under `root`. */
export function shellProblems(root: string): string[] {
  const result = spawnSync("git", ["-C", root, "ls-files", "-z", "*.sh", "*.bash", "*.bats"], {
    encoding: "utf8",
  });
  if (result.status !== 0) throw new Error(`git ls-files failed: ${result.stderr.trim()}`);
  const problems = result.stdout
    .split("\0")
    .filter((file) => file !== "" && !SHELL_KEEP.has(file))
    .map((file) => `tracked shell file: ${file}`);
  for (const [file, body] of SHIMS) {
    const path = join(root, file);
    if (!existsSync(path)) {
      problems.push(`missing native shim: ${file}`);
      continue;
    }
    if (readFileSync(path, "utf8") !== body) problems.push(`native shim changed: ${file}`);
    const source = lstatSync(path);
    if (!source.isFile()) problems.push(`native shim is not a regular file: ${file}`);
    if ((source.mode & 0o111) === 0) problems.push(`native shim not executable: ${file}`);
  }
  return problems;
}
