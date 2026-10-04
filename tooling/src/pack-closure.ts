/**
 * Closure of the published `@toolu/opencode` tarball (#361). Everything a
 * shipped file reaches must ship in the same tarball or come from a declared
 * dependency. That covers the helper paths named by the generated surfaces
 * and plugin docs, Markdown links, module imports and export targets.
 * Executable bundles must also keep their mode. Each problem is one line.
 */
import { existsSync, readFileSync, statSync } from "node:fs";
import { builtinModules } from "node:module";
import { join, posix } from "node:path";
import { z } from "zod";
import type { PackedFile } from "./npm-pack.ts";

type ClosureInput = {
  /** The package root, after prepack. */
  packageDir: string;
  files: readonly PackedFile[];
  /** `<repo>/plugins`, the source of truth for each bundle's executable bit. */
  sourcePlugins: string;
  /** Keys of `@toolu/core`'s `exports`, e.g. `"./dispatch"`. */
  coreExports: readonly string[];
};

const NAME = "@toolu/opencode";
const Manifest = z.looseObject({
  main: z.string().optional(),
  exports: z.record(z.string(), z.string()).optional(),
  dependencies: z.record(z.string(), z.string()).optional(),
});
type Manifest = z.infer<typeof Manifest>;

/** `$TOOLU_PLUGIN_ROOT[_NAME]/path` or `$TOOLU_OPENCODE_ROOT/path`, braced or not. */
const REFERENCE =
  /\$\{?(TOOLU_PLUGIN_ROOT(?:_[A-Z0-9_]+)?|TOOLU_OPENCODE_ROOT)\}?\/([^\s`"')\]]*)/g;
const LINK = /\]\(([^)\s]+)\)/g;
const PLACEHOLDER = /[<…*${]/;
const SCHEME = /^[a-z][a-z0-9+.-]*:/i;
const RESOLVE_SUFFIXES = ["", ".ts", ".js", "/index.ts", "/index.js"];
const BUILTINS = new Set(builtinModules);

/** The tarball directory a root variable names: `plugins/toolu/`, `plugins/<name>/` or the package root. */
function rootDir(variable: string): string {
  if (variable === "TOOLU_OPENCODE_ROOT") return "";
  if (variable === "TOOLU_PLUGIN_ROOT") return "plugins/toolu/";
  const plugin = variable.slice("TOOLU_PLUGIN_ROOT_".length).toLowerCase().replaceAll("_", "-");
  return `plugins/${plugin}/`;
}

function packs(paths: ReadonlySet<string>, path: string): boolean {
  if (!path.endsWith("/")) return paths.has(path);
  return [...paths].some((candidate) => candidate.startsWith(path));
}

function referenceProblems(file: string, text: string, paths: ReadonlySet<string>): string[] {
  const problems: string[] = [];
  for (const match of text.matchAll(REFERENCE)) {
    const [whole, variable = "", rawPath = ""] = match;
    const path = rawPath.replace(/[.,;:]+$/, "");
    if (path === "" || PLACEHOLDER.test(path)) continue;
    if (!packs(paths, posix.normalize(rootDir(variable) + path))) {
      problems.push(`${NAME}: ${file} references ${whole}, which the tarball does not contain`);
    }
  }
  return problems;
}

function linkProblems(file: string, text: string, paths: ReadonlySet<string>): string[] {
  const problems: string[] = [];
  for (const match of text.matchAll(LINK)) {
    const link = match[1] ?? "";
    const [target = ""] = link.split("#", 1);
    if (target === "" || SCHEME.test(target) || target.startsWith("/")) continue;
    if (!paths.has(posix.normalize(posix.join(posix.dirname(file), target)))) {
      problems.push(`${NAME}: ${file} links ${link}, which the tarball does not contain`);
    }
  }
  return problems;
}

/** `@scope/name` or `name`, and the `./sub` export key the specifier asks for. */
function splitBare(specifier: string): { pkg: string; key: string } {
  const parts = specifier.split("/");
  const size = specifier.startsWith("@") ? 2 : 1;
  const rest = parts.slice(size).join("/");
  return { pkg: parts.slice(0, size).join("/"), key: rest === "" ? "." : `./${rest}` };
}

function isBuiltin(specifier: string): boolean {
  return /^(node|bun):/.test(specifier) || specifier === "bun" || BUILTINS.has(specifier);
}

function importProblem(
  file: string,
  specifier: string,
  paths: ReadonlySet<string>,
  context: { manifest: Manifest; coreExports: ReadonlySet<string> },
): string | undefined {
  if (specifier.startsWith(".")) {
    const base = posix.normalize(posix.join(posix.dirname(file), specifier));
    if (RESOLVE_SUFFIXES.some((suffix) => paths.has(base + suffix))) return undefined;
    return `${NAME}: ${file} imports ${specifier}, which the tarball does not contain`;
  }
  if (isBuiltin(specifier)) return undefined;
  const { pkg, key } = splitBare(specifier);
  if (context.manifest.dependencies?.[pkg] === undefined) {
    return `${NAME}: ${file} imports ${specifier}, which is not a declared dependency`;
  }
  if (pkg === "@toolu/core" && !context.coreExports.has(key)) {
    return `${NAME}: ${file} imports ${specifier}, which @toolu/core does not export`;
  }
  return undefined;
}

function importProblems(
  file: string,
  text: string,
  paths: ReadonlySet<string>,
  context: { manifest: Manifest; coreExports: ReadonlySet<string> },
): string[] {
  const loader = file.endsWith(".ts") ? "ts" : "js";
  // Bun 1.4.2's scanImports throws `Unexpected #!/usr/bin/env bun` on a shebang,
  // which every executable bundle starts with, so the first line is blanked.
  const source = text.replace(/^#!.*/, "");
  const imports = new Bun.Transpiler({ loader }).scanImports(source);
  return imports.flatMap(({ path }) => importProblem(file, path, paths, context) ?? []);
}

function exportProblems(manifest: Manifest, paths: ReadonlySet<string>): string[] {
  const targets = Object.entries(manifest.exports ?? {});
  if (manifest.main !== undefined) targets.push(["main", manifest.main]);
  return targets
    .filter(([, target]) => !paths.has(posix.normalize(target)))
    .map(([key, target]) => `${NAME}: export ${key} → ${target} is not in the tarball`);
}

function modeProblems(files: readonly PackedFile[], sourcePlugins: string): string[] {
  return files.flatMap(({ path, mode }) => {
    if (!path.startsWith("plugins/") || (mode & 0o111) !== 0) return [];
    const source = join(sourcePlugins, path.slice("plugins/".length));
    if (!existsSync(source) || (statSync(source).mode & 0o111) === 0) return [];
    const octal = (mode & 0o777).toString(8);
    return [`${NAME}: ${path} lost its executable bit (mode ${octal})`];
  });
}

function fileProblems(
  path: string,
  text: string,
  paths: ReadonlySet<string>,
  context: { manifest: Manifest; coreExports: ReadonlySet<string> },
): string[] {
  const surface = path.startsWith("generated/") || path.startsWith("plugins/");
  const problems: string[] = [];
  if (surface && /\.(md|json)$/.test(path)) problems.push(...referenceProblems(path, text, paths));
  if (path.startsWith("generated/") && path.endsWith(".md")) {
    problems.push(...linkProblems(path, text, paths));
  }
  if (/\.(ts|js)$/.test(path)) problems.push(...importProblems(path, text, paths, context));
  return problems;
}

/** Every closure problem in the packed `@toolu/opencode`, one human-readable line each. */
export function closureProblems(input: ClosureInput): readonly string[] {
  const paths = new Set(input.files.map((file) => file.path));
  const manifest = Manifest.parse(
    JSON.parse(readFileSync(join(input.packageDir, "package.json"), "utf8")),
  );
  const context = { manifest, coreExports: new Set(input.coreExports) };
  const problems = input.files
    .filter(({ path }) => /\.(md|json|ts|js)$/.test(path))
    .flatMap(({ path }) =>
      fileProblems(path, readFileSync(join(input.packageDir, path), "utf8"), paths, context),
    );
  problems.push(
    ...exportProblems(manifest, paths),
    ...modeProblems(input.files, input.sourcePlugins),
  );
  return [...new Set(problems)];
}
