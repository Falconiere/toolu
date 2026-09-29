/**
 * Type-aware oxlint across every Bun workspace package that owns an
 * .oxlintrc.json, plus plugin script trees under plugins/*\/ that ship one
 * (`bun run lint:ts`). Each config lints its `src/`, else its `scripts/`; a
 * config with neither is an error. Every directory is linted even after a
 * failure, and the run fails if any did. `LINT_TS_ROOT` points it elsewhere.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const SEARCH_ROOTS = ["tooling", "packages", "tools", "plugins"];

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

function lintDir(oxlint: string, dir: string): boolean {
  const target = ["src", "scripts"].find((name) => existsSync(join(dir, name)));
  if (target === undefined) {
    console.error(`lint:ts: no src/ or scripts/ under ${dir}`);
    return false;
  }
  const res = spawnSync(
    oxlint,
    ["--type-aware", "--deny-warnings", "-c", ".oxlintrc.json", target],
    {
      cwd: dir,
      stdio: "inherit",
    },
  );
  if (res.error) throw res.error;
  return res.status === 0;
}

function main(): number {
  const root = process.env["LINT_TS_ROOT"] ?? resolve(import.meta.dir, "../..");
  const oxlint = Bun.which("oxlint") ?? join(root, "node_modules/.bin/oxlint");
  const dirs = SEARCH_ROOTS.flatMap((top) => configs(root, join(root, top)))
    .toSorted()
    .map((config) => config.slice(0, -"/.oxlintrc.json".length));
  let ok = true;
  for (const dir of dirs) {
    process.stdout.write(`lint:ts: ${dir}\n`);
    if (!lintDir(oxlint, dir)) ok = false;
  }
  return ok ? 0 : 1;
}

if (import.meta.main) process.exitCode = main();
