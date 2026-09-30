/**
 * Rules that look past the file's text (#265): the `../` import rule (10),
 * gated on a `@/` path alias in the nearest tsconfig up to the project root,
 * and test placement (20), which inspects the directory beside `__tests__/`.
 */
import { lstatSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { isJsonObject } from "@toolu/core/config";
import { isTestPath, type TsFile } from "./ts-file.ts";

/**
 * `jq -e '[(.compilerOptions.paths // {}) | keys[] | select(startswith("@/"))] | length > 0'`:
 * any error (bad JSON, a non-object where jq indexes) is "no alias".
 */
function declaresAtAlias(config: string): boolean {
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(config, "utf8"));
  } catch {
    return false;
  }
  if (doc !== null && !isJsonObject(doc)) return false;
  const options = doc === null ? null : (doc.compilerOptions ?? null);
  if (options !== null && !isJsonObject(options)) return false;
  const paths = options === null ? null : (options.paths ?? null);
  if (paths === null || paths === false) return false;
  return isJsonObject(paths) && Object.keys(paths).some((key) => key.startsWith("@/"));
}

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** `_ts_has_at_alias`: walk from the file's directory up to the project root (or `/`). */
function hasAtAlias(f: TsFile): boolean {
  const root = resolve(f.ctx.projectRoot);
  let dir = dirname(f.file.absolute);
  for (;;) {
    for (const name of ["tsconfig.json", "tsconfig.base.json"]) {
      const config = join(dir, name);
      if (isFile(config) && declaresAtAlias(config)) return true;
    }
    if (dir === root || dir === "/") return false;
    dir = dirname(dir);
  }
}

/** 10-imports. */
export function parentImport(f: TsFile): string | undefined {
  if (!f.lines.some((line) => /from ["']\.\.\//s.test(line))) return undefined;
  if (!hasAtAlias(f)) return undefined;
  return `Forbidden ../ import in ${f.file.path} — use @/ alias`;
}

function entries(dir: string): string[] {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

function kind(path: string): "file" | "dir" | "other" {
  try {
    const stat = lstatSync(path);
    return stat.isFile() ? "file" : stat.isDirectory() ? "dir" : "other";
  } catch {
    return "other";
  }
}

/** `find DIR -maxdepth 1 -type f` for a non-test `.ts`/`.tsx` source. */
function hasSource(dir: string): boolean {
  return entries(dir).some(
    (name) =>
      /\.tsx?$/s.test(name) &&
      !name.includes(".test.") &&
      !name.includes(".spec.") &&
      !name.endsWith(".d.ts") &&
      kind(join(dir, name)) === "file",
  );
}

/** `find DIR -maxdepth 1 -type d ! -name __tests__ ! -name .`: DIR itself counts too. */
function hasOtherDir(dir: string, absolute: string): boolean {
  const own = dir === "/" ? "/" : basename(dir);
  if (kind(absolute) === "dir" && own !== "__tests__" && own !== ".") return true;
  return entries(absolute).some(
    (name) => name !== "__tests__" && kind(join(absolute, name)) === "dir",
  );
}

/** 20-tests: test files live flat in a `__tests__/` beside the code they test. */
export function testPlacement(f: TsFile): string[] {
  const path = f.file.path;
  if (!isTestPath(path) || path.includes("/e2e/")) return [];
  if (!path.includes("/__tests__/")) {
    return [`Test file outside __tests__/: ${path} — move to sibling __tests__/ directory`];
  }
  const errors: string[] = [];
  const cut = path.lastIndexOf("__tests__/");
  const after = path.slice(cut + "__tests__/".length);
  const sub = after.includes("/") ? after.slice(0, after.indexOf("/")) : undefined;
  if (sub !== undefined && sub !== "fixtures" && sub !== "helpers" && sub !== "utils") {
    errors.push(
      `Test nested in __tests__/ subdirectory: ${path} — keep __tests__/ flat (only fixtures/helpers/utils subdirs allowed; no mocks/ — tests must exercise real data)`,
    );
  }
  const parent = dirname(`${path.slice(0, cut)}__tests__`);
  const absolute = resolve(f.ctx.cwd ?? process.cwd(), parent);
  if (!hasSource(absolute) && !hasOtherDir(parent, absolute)) {
    errors.push(
      `Test not co-located with source: ${path} — __tests__/ must be at the same level as the code it tests`,
    );
  }
  return errors;
}
