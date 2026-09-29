/**
 * Duplicate exported types (50) and throws of non-Error literals (80), #265.
 * The duplicate lookup is the same `git grep` the bash fragment ran, so git's
 * index and pathspec rules decide which files count.
 */
import { spawnSync } from "node:child_process";
import { head, spawnEnv, type TsFile } from "./ts-file.ts";

const PATHSPECS = ["packages/*.ts", "packages/*.tsx", "apps/*.ts", "apps/*.tsx"];

/** `git grep -l --untracked` for another file exporting `name`, or "". */
function definedElsewhere(f: TsFile, name: string, relative: string): string {
  const pattern = `^export (interface|type) ${name}[ <{]`;
  const res = spawnSync(
    "git",
    ["-C", f.ctx.projectRoot, "grep", "-l", "--untracked", "-E", pattern, "--", ...PATHSPECS],
    { cwd: f.ctx.cwd, env: spawnEnv(f), encoding: "utf8" },
  );
  const files = (res.stdout ?? "").split("\n").filter((line) => line !== "" && line !== relative);
  return files[0] ?? "";
}

/** 50-type-dup: one error per exported type name another package already exports. */
export function duplicateTypes(f: TsFile): string[] {
  const names = f.lines.flatMap((line) => {
    const match = /^export (?:interface|type) ([A-Z][a-zA-Z]+)/s.exec(line);
    return match?.[1] === undefined ? [] : [match[1]];
  });
  if (names.length === 0) return [];
  const prefix = `${f.ctx.projectRoot}/`;
  const path = f.file.path;
  const relative = path.startsWith(prefix) ? path.slice(prefix.length) : path;
  return names.flatMap((name) => {
    const other = definedElsewhere(f, name, relative);
    return other === ""
      ? []
      : [`Type '${name}' in ${path} already defined in ${other} — import instead of redefining`];
  });
}

const THROW_LITERAL =
  /(^|[^a-zA-Z_$])throw[ \t]+(-?[0-9]+(\.[0-9]+)?|null|undefined|true|false)([ \t]|;|}|$)/s;

/** 80-throw-literal: `/* … *\/` and `//` comments are stripped before matching. */
export function throwLiteral(f: TsFile): string | undefined {
  const rows = f.lines.flatMap((line, i) => {
    const code = line.replaceAll(/\/\*.*\*\//gs, "").replace(/\/\/.*$/s, "");
    return THROW_LITERAL.test(code) ? [`${String(i + 1)}: ${line}`] : [];
  });
  const excerpt = head(rows, 3);
  if (excerpt === "") return undefined;
  return `throw of non-Error literal in ${f.file.path} — throw an Error (or subclass) instead\n${excerpt}`;
}
