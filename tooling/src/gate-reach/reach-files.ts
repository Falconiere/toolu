/** The file universe of the gate-reach checks, and the glob tests they share. */
import { spawnSync } from "node:child_process";
import { fatal } from "./reach-schema.ts";

export function matches(glob: string, path: string): boolean {
  return new Bun.Glob(glob).match(path);
}

/** `pattern` names `path` itself or a directory above it. */
export function covers(pattern: string, path: string): boolean {
  return matches(pattern, path) || matches(`${pattern}/**`, path);
}

/**
 * An ignore entry naming one file is a legacy exemption, not a structural
 * rule. jscpd only honours such an entry behind a leading `**\/`, so one is
 * dropped before looking for glob characters.
 */
export function isExactPath(pattern: string): boolean {
  return !/[*?[\]{}!]/.test(pattern.replace(/^\*\*\//, ""));
}

/** The repo path an exact-path entry names. */
export function exactPath(pattern: string): string {
  return pattern.replace(/^\*\*\//, "");
}

/** Tracked TypeScript files outside `exclude`. No work tree, or none left, is fatal. */
export function trackedFiles(root: string, exclude: readonly string[]): string[] {
  const res = spawnSync("git", ["-C", root, "ls-files", "-z", "--", "*.ts", "*.tsx"], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (res.error !== undefined || res.status !== 0) {
    fatal(`git ls-files failed in ${root}: ${res.error?.message ?? res.stderr.trim()}`);
  }
  const files = res.stdout
    .split("\0")
    .filter((file) => file !== "" && !exclude.some((glob) => matches(glob, file)));
  if (files.length === 0) {
    fatal("no tracked TypeScript files after exclude: a gate that governs nothing must not pass");
  }
  return files;
}
