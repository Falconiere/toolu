/**
 * Asserts each published tarball's file list, as `npm pack` (the tool
 * `npm publish` uses, prepack included) reports it.
 *
 * `@toolu/plugins`, the `toolu` CLI, ships a Node bundle and its bundled
 * marketplace manifest and nothing else — above all not the bash plugins/ tree,
 * which Claude Code and Codex users never read from npm. It packs from
 * tools/toolu-cli/npm, not the workspace, so no local package shares its name.
 *
 * `@toolu/opencode` stages manifests, settings and committed hook bundles, but
 * never the repository's bash source tree or its tests. It packs from a temp
 * copy (`stageOpencode`), and its closure gate (`pack-closure.ts`) requires
 * every helper, link, import and export target it reaches to ship with it.
 * Files reached only through code or a shell variable such as `$ROOT` stay
 * listed in `required`.
 */
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { z } from "zod";
import { committedBundles } from "./build-plugins.ts";
import { packedFiles, stageOpencode } from "./npm-pack.ts";
import { closureProblems } from "./pack-closure.ts";

const ROOT = resolve(import.meta.dir, "../..");

/** A plugin's TypeScript hook sources: they ship built into hooks/dist, never in source form. */
export const HOOK_SOURCES = /(^|\/)hooks\/src\//;
/** Tests and their fixtures import the private harness, so no installed copy can run them. */
export const TEST_FILES = /(^|\/)(__tests__|fixtures)\//;

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
        "src/bootstrap/native-launcher.txt",
        "plugins/toolu/hooks/hooks.json",
        "plugins/rust-quality/.claude-plugin/plugin.json",
        "plugins/toolu/settings/protected-files.txt",
        "plugins/epic-orchestrator/scripts/launch-issue.ts",
        "plugins/epic-orchestrator/scripts/epic-watch.ts",
        "plugins/epic-orchestrator/scripts/trackers/jira.ts",
        "plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md",
        "plugins/pr-babysit/skills/babysit/references/fixer-brief.md",
        ...committedBundles(root),
      ],
      forbidden: ["node_modules/", ".env"],
      forbiddenPatterns: [HOOK_SOURCES, TEST_FILES, /\.(?:sh|bash|bats)$/],
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

const CoreManifest = z.looseObject({ exports: z.record(z.string(), z.unknown()) });

/** Inventory and closure problems of `@toolu/opencode`, packed from a temp copy through its own prepack. */
function opencodeProblems(expectation: Expectation): string[] {
  const work = mkdtempSync(join(tmpdir(), "pack-inventory-opencode-"));
  try {
    const stage = stageOpencode(work, ROOT);
    const files = packedFiles(stage);
    const core = CoreManifest.parse(
      JSON.parse(readFileSync(join(ROOT, "packages/toolu-core/package.json"), "utf8")),
    );
    process.stdout.write(`pack-inventory: ${expectation.name} — ${files.length} files\n`);
    return [
      ...checkOne(
        expectation,
        files.map((file) => file.path),
      ),
      ...closureProblems({
        packageDir: stage,
        files,
        sourcePlugins: join(ROOT, "plugins"),
        coreExports: Object.keys(core.exports),
      }),
    ];
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

function run(): number {
  const problems: string[] = [];
  for (const expectation of expectations(ROOT)) {
    if (expectation.name === "@toolu/opencode") {
      problems.push(...opencodeProblems(expectation));
      continue;
    }
    const files = packedFiles(expectation.dir).map((file) => file.path);
    problems.push(...checkOne(expectation, files));
    process.stdout.write(`pack-inventory: ${expectation.name} — ${files.length} files\n`);
  }
  for (const problem of problems) process.stderr.write(`RED  ${problem}\n`);
  if (problems.length === 0) process.stdout.write("pack-inventory: ok\n");
  return problems.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exitCode = run();
