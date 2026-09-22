/**
 * Asserts each published tarball's file list.
 *
 * The `toolu` CLI ships a Node bundle and its bundled marketplace manifest and
 * nothing else — above all not the bash plugins/ tree, which Claude Code and
 * Codex users never read from npm.
 *
 * `@toolu/opencode` is the one package that DOES carry that tree, because its
 * OpenCode bridge enforces the bash gates and npm cannot reach outside a package
 * directory. Its prepack stages the copy.
 */
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../..");

interface Expectation {
  readonly dir: string;
  readonly name: string;
  readonly required: readonly string[];
  readonly forbidden: readonly string[];
  readonly exact: boolean;
}

const EXPECTED: readonly Expectation[] = [
  {
    dir: "tools/toolu-cli",
    name: "@toolu/cli",
    required: ["package.json", "README.md", "LICENSE", "assets/marketplace.json", "dist/cli.js"],
    forbidden: ["plugins/", "src/", "node_modules/", ".env"],
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
      "src/runner/runner.ts",
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
    ],
    forbidden: ["node_modules/", ".env"],
    exact: false,
  },
];

function packedFiles(directory: string): readonly string[] {
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

function checkOne(expectation: Expectation, files: readonly string[]): readonly string[] {
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
  if (expectation.exact) {
    const allowed = new Set(expectation.required);
    for (const file of files.filter((candidate) => !allowed.has(candidate))) {
      problems.push(`${expectation.name} tarball has an undeclared file: ${file}`);
    }
  }
  return problems;
}

function run(): number {
  const problems: string[] = [];
  for (const expectation of EXPECTED) {
    const files = packedFiles(expectation.dir);
    problems.push(...checkOne(expectation, files));
    process.stdout.write(`pack-inventory: ${expectation.name} — ${files.length} files\n`);
  }
  for (const problem of problems) process.stderr.write(`RED  ${problem}\n`);
  if (problems.length === 0) process.stdout.write("pack-inventory: ok\n");
  return problems.length === 0 ? 0 : 1;
}

process.exitCode = run();
