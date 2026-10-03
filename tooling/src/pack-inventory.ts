/**
 * Asserts each published tarball's file list.
 *
 * `@toolu/plugins`, the `toolu` CLI, ships a Node bundle and its bundled
 * marketplace manifest and nothing else — above all not the bash plugins/ tree,
 * which Claude Code and Codex users never read from npm. It packs from
 * tools/toolu-cli/npm, not the workspace, so no local package shares its name.
 *
 * `@toolu/opencode` stages manifests, settings and committed hook bundles, but
 * never the repository's bash source tree.
 */
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { committedBundles } from "./build-plugins.ts";

const ROOT = resolve(import.meta.dir, "../..");

/** A plugin's TypeScript hook sources: they ship built into hooks/dist, never in source form. */
export const HOOK_SOURCES = /(^|\/)hooks\/src\//;

export interface Expectation {
  readonly dir: string;
  readonly name: string;
  readonly required: readonly string[];
  readonly forbidden: readonly string[];
  readonly forbiddenPatterns?: readonly RegExp[];
  readonly exact: boolean;
}

/** Every published tarball's expected file list, given the repository at `root`. */
export function expectations(root: string): readonly Expectation[] {
  return [
    {
      dir: "tools/toolu-cli/npm",
      name: "@toolu/plugins",
      required: ["package.json", "README.md", "LICENSE", "assets/marketplace.json", "dist/cli.js"],
      forbidden: ["plugins/", "src/", "node_modules/", ".env"],
      forbiddenPatterns: [HOOK_SOURCES],
      exact: true,
    },
    {
      dir: "packages/toolu-core",
      name: "@toolu/core",
      required: [
        "package.json",
        "README.md",
        "LICENSE",
        "src/decision/decision.ts",
        "src/dispatch/dispatch.ts",
      ],
      forbidden: ["plugins/", "dist/", "node_modules/", ".env"],
      exact: false,
    },
    {
      dir: "tools/toolu-opencode",
      name: "@toolu/opencode",
      required: [
        "package.json",
        "README.md",
        "LICENSE",
        "src/plugin/toolu.ts",
        "plugins/toolu/hooks/hooks.json",
        "plugins/rust-quality/.claude-plugin/plugin.json",
        "plugins/toolu/settings/protected-files.txt",
        "plugins/toolu/scripts/debug-io.ts",
        "plugins/toolu/scripts/debug-log.ts",
        "plugins/toolu/scripts/debug-stack.ts",
        "plugins/toolu/scripts/debug-testfail.ts",
        ...committedBundles(root),
      ],
      forbidden: ["node_modules/", ".env"],
      forbiddenPatterns: [HOOK_SOURCES, /\.(?:sh|bash|bats)$/],
      exact: false,
    },
  ];
}

/** The file list `bun pm pack` would publish from `directory` (absolute, or relative to the repo root). */
export function packedFiles(directory: string): readonly string[] {
  const result = spawnSync("bun", ["pm", "pack", "--dry-run"], {
    cwd: resolve(ROOT, directory),
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`bun pm pack --dry-run failed in ${directory}: ${result.stderr}`);
  }
  return result.stdout
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith("packed "))
    .map((line) => line.replace(/^packed \S+ /, ""));
}

/** Every way `files` breaks `expectation`, one human-readable problem per line. */
export function checkOne(expectation: Expectation, files: readonly string[]): readonly string[] {
  const problems: string[] = [];
  for (const required of expectation.required) {
    if (!files.includes(required)) {
      problems.push(`${expectation.name} tarball is missing ${required}`);
    }
  }
  for (const forbidden of expectation.forbidden) {
    for (const file of files.filter((candidate) => candidate.startsWith(forbidden))) {
      problems.push(`${expectation.name} tarball must not contain ${file}`);
    }
  }
  for (const pattern of expectation.forbiddenPatterns ?? []) {
    for (const file of files.filter((candidate) => pattern.test(candidate))) {
      problems.push(`${expectation.name} tarball must not contain ${file}`);
    }
  }
  if (expectation.exact) {
    const allowed = new Set(expectation.required);
    for (const file of files.filter((candidate) => !allowed.has(candidate))) {
      problems.push(`${expectation.name} tarball has an undeclared file: ${file}`);
    }
  }
  // A file can break a prefix rule and a pattern rule at once; report it once.
  return [...new Set(problems)];
}

function run(): number {
  const problems: string[] = [];
  for (const expectation of expectations(ROOT)) {
    const files = packedFiles(expectation.dir);
    problems.push(...checkOne(expectation, files));
    process.stdout.write(`pack-inventory: ${expectation.name} — ${files.length} files\n`);
  }
  for (const problem of problems) process.stderr.write(`RED  ${problem}\n`);
  if (problems.length === 0) process.stdout.write("pack-inventory: ok\n");
  return problems.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exitCode = run();
