/**
 * Type-aware oxlint across every directory that owns an .oxlintrc.json
 * (`bun run lint:ts`). A package config lints each of its `src/`, `scripts/`
 * and `contract/`; a collection config with none of those (`plugins/`) lints
 * `hooks/src/`, `scripts/` and `skills/` of every child directory. A config
 * with no target is an error. Every directory is linted even after a failure,
 * and the run fails if any did. `LINT_TS_ROOT` points it elsewhere.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { envOr } from "./env.ts";

const SEARCH_ROOTS = ["tooling", "packages", "tools", "plugins"];
const PACKAGE_TARGETS = ["src", "scripts", "contract"];
const COLLECTION_TARGETS = ["hooks/src", "scripts", "skills"];

/** One lint run: the directory holding the config, and its targets relative to it. */
export type LintDir = { readonly dir: string; readonly targets: readonly string[] };

/** Every .oxlintrc.json below `dir`, skipping node_modules and the OpenCode plugin mirror. */
function configs(root: string, dir: string): string[] {
  if (!existsSync(dir) || dir === join(root, "tools/toolu-opencode/plugins")) return [];
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory() && entry.name !== "node_modules") out.push(...configs(root, path));
    else if (entry.isFile() && entry.name === ".oxlintrc.json") out.push(path);
  }
  return out;
}

/** The package targets `dir` has, else the collection targets of each child directory. */
function targetsOf(dir: string): string[] {
  const own = PACKAGE_TARGETS.filter((name) => existsSync(join(dir, name)));
  if (own.length > 0) return own;
  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name !== "node_modules")
    .map((entry) => entry.name)
    .toSorted()
    .flatMap((child) => COLLECTION_TARGETS.map((target) => `${child}/${target}`))
    .filter((target) => existsSync(join(dir, target)));
}

/** Every config directory under `root` with its lint targets; what `lint:ts` runs, in order. */
export function lintDirs(root: string): LintDir[] {
  return SEARCH_ROOTS.flatMap((top) => configs(root, join(root, top)))
    .toSorted()
    .map((config) => config.slice(0, -"/.oxlintrc.json".length))
    .map((dir) => ({ dir, targets: targetsOf(dir) }));
}

function lintDir(oxlint: string, { dir, targets }: LintDir): boolean {
  if (targets.length === 0) {
    console.error(`lint:ts: no lint target under ${dir}`);
    return false;
  }
  const res = spawnSync(
    oxlint,
    ["--type-aware", "--deny-warnings", "-c", ".oxlintrc.json", ...targets],
    {
      cwd: dir,
      stdio: "inherit",
    },
  );
  if (res.error) throw res.error;
  return res.status === 0;
}

function main(): number {
  const root = envOr("LINT_TS_ROOT", resolve(import.meta.dir, "../.."));
  const oxlint = Bun.which("oxlint") ?? join(root, "node_modules/.bin/oxlint");
  let ok = true;
  for (const entry of lintDirs(root)) {
    process.stdout.write(`lint:ts: ${entry.dir}\n`);
    if (!lintDir(oxlint, entry)) ok = false;
  }
  return ok ? 0 : 1;
}

if (import.meta.main) process.exitCode = main();
