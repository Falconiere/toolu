/**
 * Asserts each published tarball's file list, as `npm pack` (the tool
 * `npm publish` uses, prepack included) reports it.
 *
 * `@toolu/plugins`, the `toolu-plugins` wrapper that runs the native
 * `toolu plugins`, ships a Node bundle and its bundled marketplace manifest and
 * nothing else — above all not the bash plugins/ tree,
 * which Claude Code and Codex users never read from npm. It packs from
 * tools/toolu-cli/npm, not the workspace, so no local package shares its name.
 */
import { packedFiles } from "./npm-pack.ts";

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

/** Every published tarball's expected file list. */
export function expectations(): readonly Expectation[] {
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
  ];
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
  for (const expectation of expectations()) {
    const files = packedFiles(expectation.dir).map((file) => file.path);
    problems.push(...checkOne(expectation, files));
    process.stdout.write(`pack-inventory: ${expectation.name} — ${files.length} files\n`);
  }
  for (const problem of problems) process.stderr.write(`RED  ${problem}\n`);
  if (problems.length === 0) process.stdout.write("pack-inventory: ok\n");
  return problems.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exitCode = run();
