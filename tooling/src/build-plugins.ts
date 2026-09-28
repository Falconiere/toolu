/**
 * Bundles each plugin's TypeScript hook entries into committed single-file ESM.
 *
 * Every top-level `plugins/<name>/hooks/src/*.ts` (not `*.test.ts`, not `*.d.ts`)
 * is an entry; subdirectories hold helpers that only reach a bundle through an
 * import. Output lands in `plugins/<name>/hooks/dist/<entry>.js`, inlines
 * `@toolu/core` and its dependencies, and runs under `bun` with no node_modules.
 *
 * `bun build` writes each bundled module's path into a comment relative to the
 * process cwd (the `root` option does not change that), so every build runs with
 * cwd pinned to the repository root: that is what makes output byte-identical
 * across machines, checkouts and callers.
 *
 * Usage: bun run tooling/src/build-plugins.ts [--check] [--root <dir>]
 */
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

export interface BundleEntry {
  readonly plugin: string;
  readonly name: string;
  /** Repository-relative path of the entry source. */
  readonly source: string;
}

export interface DriftProblem {
  readonly kind: "drift" | "missing" | "orphan";
  /** Repository-relative path of the committed bundle. */
  readonly path: string;
}

/** Sorted names of a directory's subdirectories (or files); none when it is absent. */
function childNames(dir: string, directories: boolean): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((item) => item.isDirectory() === directories)
    .map((item) => item.name)
    .toSorted();
}

function isEntry(name: string): boolean {
  return name.endsWith(".ts") && !name.endsWith(".test.ts") && !name.endsWith(".d.ts");
}

function distPath(plugin: string, name: string): string {
  return `plugins/${plugin}/hooks/dist/${name}.js`;
}

/** Every bundle entry in the tree, sorted by plugin then entry name. */
export function discoverEntries(root: string): BundleEntry[] {
  return childNames(join(root, "plugins"), true).flatMap((plugin) =>
    childNames(join(root, "plugins", plugin, "hooks/src"), false)
      .filter(isEntry)
      .map((file) => ({
        plugin,
        name: file.slice(0, -".ts".length),
        source: `plugins/${plugin}/hooks/src/${file}`,
      })),
  );
}

function bundle(root: string, entry: BundleEntry, outfile: string): void {
  const args = ["build", entry.source, "--target", "bun", "--format", "esm"];
  args.push("--sourcemap=none", "--outfile", outfile);
  const result = spawnSync(process.execPath, args, { cwd: root, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`bun build failed for ${entry.source}:\n${result.stderr}${result.stdout}`);
  }
}

/** Bundles every entry into `<outDir>/<plugin>/<entry>.js`; throws on the first failure. */
export function stageBundles(root: string, outDir: string): BundleEntry[] {
  const entries = discoverEntries(root);
  for (const entry of entries) bundle(root, entry, join(outDir, entry.plugin, `${entry.name}.js`));
  return entries;
}

function withStaging<T>(root: string, use: (staged: string, entries: BundleEntry[]) => T): T {
  const staged = mkdtempSync(join(tmpdir(), "toolu-bundles-"));
  try {
    return use(staged, stageBundles(root, staged));
  } finally {
    rmSync(staged, { recursive: true, force: true });
  }
}

/** Repository-relative paths of the committed bundles in the tree, sorted. */
export function committedBundles(root: string): string[] {
  return childNames(join(root, "plugins"), true).flatMap((plugin) =>
    childNames(join(root, "plugins", plugin, "hooks/dist"), false)
      .filter((file) => file.endsWith(".js"))
      .map((file) => `plugins/${plugin}/hooks/dist/${file}`),
  );
}

/** Rebuilds every committed bundle; nothing in the tree changes unless every entry builds. */
export function buildPlugins(root: string): BundleEntry[] {
  return withStaging(root, (staged, entries) => {
    const wanted = new Set(entries.map((entry) => distPath(entry.plugin, entry.name)));
    for (const path of committedBundles(root).filter((candidate) => !wanted.has(candidate))) {
      rmSync(join(root, path));
    }
    for (const entry of entries) {
      mkdirSync(join(root, "plugins", entry.plugin, "hooks/dist"), { recursive: true });
      copyFileSync(
        join(staged, entry.plugin, `${entry.name}.js`),
        join(root, distPath(entry.plugin, entry.name)),
      );
    }
    for (const plugin of childNames(join(root, "plugins"), true)) {
      const dist = join(root, "plugins", plugin, "hooks/dist");
      if (existsSync(dist) && readdirSync(dist).length === 0) rmSync(dist, { recursive: true });
    }
    return entries;
  });
}

/** Rebuilds into a temp dir and reports every committed bundle that disagrees. */
export function checkPluginBundles(root: string): DriftProblem[] {
  return withStaging(root, (staged, entries) => {
    const problems: DriftProblem[] = [];
    const wanted = new Set<string>();
    for (const entry of entries) {
      const path = distPath(entry.plugin, entry.name);
      wanted.add(path);
      if (!existsSync(join(root, path))) {
        problems.push({ kind: "missing", path });
      } else if (
        !readFileSync(join(root, path)).equals(
          readFileSync(join(staged, entry.plugin, `${entry.name}.js`)),
        )
      ) {
        problems.push({ kind: "drift", path });
      }
    }
    for (const path of committedBundles(root).filter((candidate) => !wanted.has(candidate))) {
      problems.push({ kind: "orphan", path });
    }
    return problems.toSorted((a, b) => a.path.localeCompare(b.path));
  });
}

function parseArgs(argv: readonly string[]): { check: boolean; root: string } {
  const at = argv.indexOf("--root");
  const rootArg = at === -1 ? undefined : argv[at + 1];
  if (at !== -1 && rootArg === undefined) throw new Error("--root needs a directory");
  return {
    check: argv.includes("--check"),
    root: resolve(rootArg ?? resolve(import.meta.dir, "../..")),
  };
}

function main(argv: readonly string[]): number {
  const { check, root } = parseArgs(argv);
  if (!check) {
    const entries = buildPlugins(root);
    process.stdout.write(`build-plugins: ${entries.length} bundles\n`);
    return 0;
  }
  const problems = checkPluginBundles(root);
  for (const problem of problems) process.stderr.write(`RED  ${problem.kind} ${problem.path}\n`);
  if (problems.length > 0) {
    process.stderr.write(
      "check:plugin-bundles: run `bun run build:plugins` and commit hooks/dist\n",
    );
    return 1;
  }
  process.stdout.write(`check:plugin-bundles: ok (${discoverEntries(root).length} bundles)\n`);
  return 0;
}

if (import.meta.main) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error: unknown) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
