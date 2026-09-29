/**
 * The project facts toolu's SessionStart context names (#263), ported from
 * the `detect_*` helpers of `detect.sh` it used. Private to this plugin until
 * the core detect layer (#254) lands.
 */
import { statSync } from "node:fs";
import { basename, join } from "node:path";

type Env = Record<string, string | undefined>;

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function anyFile(root: string, names: readonly string[]): boolean {
  return names.some((name) => isFile(join(root, name)));
}

export type ProjectFacts = {
  name: string;
  nodePm: string;
  rust: boolean;
  ts: boolean;
  python: boolean;
};

const LOCKFILES: readonly (readonly [string, string])[] = [
  ["bun.lock", "bun"],
  ["bun.lockb", "bun"],
  ["pnpm-lock.yaml", "pnpm"],
  ["yarn.lock", "yarn"],
  ["package-lock.json", "npm"],
];

/** `git ls-files '**\/tsconfig*.json' 'tsconfig*.json'` lists anything. */
function tracksTsconfig(root: string, env: Env): boolean {
  const res = Bun.spawnSync(
    ["git", "-C", root, "ls-files", "**/tsconfig*.json", "tsconfig*.json"],
    { env, stdout: "pipe", stderr: "ignore" },
  );
  return res.exitCode === 0 && res.stdout.length > 0;
}

/**
 * Facts for git toplevel `root`; all empty outside a repository (`root` is "").
 * `ts` costs a `git ls-files`, so it is only looked up when `withTs` asks.
 */
export function projectFacts(root: string, env: Env, withTs: boolean): ProjectFacts {
  if (root === "") return { name: "", nodePm: "", rust: false, ts: false, python: false };
  return {
    name: basename(root),
    nodePm: LOCKFILES.find(([file]) => isFile(join(root, file)))?.[1] ?? "",
    rust: isFile(join(root, "Cargo.toml")),
    ts: withTs && tracksTsconfig(root, env),
    python: anyFile(root, ["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"]),
  };
}
