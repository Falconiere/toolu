/** The file universe of the gate-reach checks, and the glob tests they share. */
import { spawnSync } from "node:child_process";
import { fatal } from "./reach-schema.ts";

export function matches(glob: string, path: string): boolean {
  return new Bun.Glob(glob).match(path);
}

/**
 * The patterns of `source`, refused when one is negated. A negation cannot be
 * decided one file at a time, and a lazy match would let an earlier glob hide it.
 */
export function plainGlobs(source: string, globs: readonly string[]): readonly string[] {
  const negated = globs.find((glob) => glob.startsWith("!"));
  if (negated !== undefined) {
    fatal(
      `${source}: negated pattern "${negated}" is not supported; reach cannot be decided for it`,
    );
  }
  return globs;
}

/** `pattern` names `path` itself or a directory above it. */
export function covers(pattern: string, path: string): boolean {
  return matches(pattern, path) || matches(`${pattern}/**`, path);
}

/** An entry with no glob characters names one file: a legacy exemption, not a structural rule. */
export function isExactPath(pattern: string): boolean {
  return !/[*?[\]{}!]/.test(pattern);
}

/**
 * .jscpd.json `ignore`, split into structural globs and exempted repo paths.
 * jscpd matches against absolute paths, so its per-file exemption is written
 * `**\/<repo path>`; a bare path would look like an exemption and exempt
 * nothing, so it is fatal.
 */
export function jscpdIgnore(ignore: readonly string[]): { structural: string[]; exempt: string[] } {
  const structural: string[] = [];
  const exempt: string[] = [];
  for (const entry of ignore) {
    const rest = entry.startsWith("**/") ? entry.slice("**/".length) : "";
    if (rest !== "" && isExactPath(rest)) exempt.push(rest);
    else if (isExactPath(entry)) {
      fatal(
        `.jscpd.json: ignore entry "${entry}" is a bare path, which jscpd does not honour; write "**/${entry}"`,
      );
    } else structural.push(entry);
  }
  return { structural, exempt };
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
