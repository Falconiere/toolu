/**
 * Skill names the pinned host already discovers (#345), so toolu never adds a
 * second skill with the same name: the host would keep whichever parse finished
 * last. Mirrors `skill/index.ts` of `opencode-ai@1.18.34`, root for root:
 *
 * 1. `$HOME/.claude/skills` and `$HOME/.agents/skills`, then each `.claude` and
 *    `.agents` from the instance directory up to the worktree (`skills/**`, dot
 *    entries included). `OPENCODE_DISABLE_EXTERNAL_SKILLS` drops all of them;
 *    `OPENCODE_DISABLE_CLAUDE_CODE[_SKILLS]` drops the `.claude` ones. These are
 *    Effect booleans (`true`, `yes`, `on`, `1`, `y`).
 * 2. Each config directory's `{skill,skills}/**`, dot entries excluded: the XDG
 *    `opencode` directory, every `.opencode` up to the worktree (unless
 *    `OPENCODE_DISABLE_PROJECT_CONFIG` is `true` or `1`), `$HOME/.opencode` and
 *    `OPENCODE_CONFIG_DIR`.
 * 3. Each configured `skills.paths` entry (`**`, dot entries excluded), with
 *    `~/` expanded and a relative path resolved against the instance directory.
 *
 * Symlinks are followed; a directory already visited (by real path) is skipped,
 * so a link loop ends. Unreadable roots are skipped. Remote `skills.urls` are
 * pulled by the host after this runs and cannot be seen here.
 */
import { readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join } from "node:path";
import { frontmatterName } from "./frontmatter.ts";

export type SkillScanScope = {
  directory: string;
  /** The host's raw worktree: `/` for a project outside version control. */
  worktree: string;
  env: Record<string, string>;
  configuredPaths: readonly string[];
};

type Root = { dir: string; dot: boolean };

const EFFECT_TRUE = new Set(["true", "yes", "on", "1", "y"]);

function effectFlag(env: Record<string, string>, key: string): boolean {
  const value = env[key];
  return value !== undefined && EFFECT_TRUE.has(value);
}

function truthyFlag(env: Record<string, string>, key: string): boolean {
  const value = env[key]?.toLowerCase();
  return value === "true" || value === "1";
}

function nonEmpty(value: string | undefined): string | undefined {
  return value === undefined || value === "" ? undefined : value;
}

function isDir(path: string): boolean {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}

/** `start` and each parent up to `stop` (inclusive) or the filesystem root. */
function upward(start: string, stop: string): string[] {
  const out: string[] = [];
  let current = start;
  for (;;) {
    out.push(current);
    if (current === stop) return out;
    const parent = dirname(current);
    if (parent === current) return out;
    current = parent;
  }
}

/** Each existing `<dir>/<target>` from `start` up to `stop`, nearest first. */
function upwardTargets(start: string, stop: string, targets: readonly string[]): string[] {
  return upward(start, stop).flatMap((dir) =>
    targets.map((target) => join(dir, target)).filter(isDir),
  );
}

function externalRoots(scope: SkillScanScope, home: string): Root[] {
  const { env } = scope;
  if (effectFlag(env, "OPENCODE_DISABLE_EXTERNAL_SKILLS")) return [];
  const claude =
    effectFlag(env, "OPENCODE_DISABLE_CLAUDE_CODE") ||
    effectFlag(env, "OPENCODE_DISABLE_CLAUDE_CODE_SKILLS");
  const targets = claude ? [".agents"] : [".claude", ".agents"];
  const global = targets.map((target) => join(home, target));
  const project = upwardTargets(scope.directory, scope.worktree, targets);
  return [...global, ...project].map((dir) => ({ dir: join(dir, "skills"), dot: true }));
}

function configRoots(scope: SkillScanScope, home: string): Root[] {
  const { env } = scope;
  // xdg-basedir, as the host uses it: an empty value counts as unset.
  const xdg = nonEmpty(env.XDG_CONFIG_HOME) ?? join(home, ".config");
  const project = truthyFlag(env, "OPENCODE_DISABLE_PROJECT_CONFIG")
    ? []
    : upwardTargets(scope.directory, scope.worktree, [".opencode"]);
  const custom = env.OPENCODE_CONFIG_DIR;
  const dirs = [join(xdg, "opencode"), ...project, join(home, ".opencode")];
  if (custom !== undefined && custom !== "") dirs.push(custom);
  return [...new Set(dirs)].flatMap((dir) => [
    { dir: join(dir, "skill"), dot: false },
    { dir: join(dir, "skills"), dot: false },
  ]);
}

function configuredRoots(scope: SkillScanScope, home: string): Root[] {
  return scope.configuredPaths.map((item) => {
    const expanded = item.startsWith("~/") ? join(home, item.slice(2)) : item;
    return { dir: isAbsolute(expanded) ? expanded : join(scope.directory, expanded), dot: false };
  });
}

/** Every `SKILL.md` below `dir`, depth first in name order. */
function skillFiles(dir: string, dot: boolean, visited: Set<string>, out: string[]): void {
  let real: string;
  let names: string[];
  try {
    real = realpathSync(dir);
    names = readdirSync(dir).toSorted();
  } catch {
    return;
  }
  if (visited.has(real)) return;
  visited.add(real);
  for (const name of names) {
    if (!dot && name.startsWith(".")) continue;
    const path = join(dir, name);
    if (isDir(path)) skillFiles(path, dot, visited, out);
    else if (name === "SKILL.md") out.push(path);
  }
}

function readName(path: string): string | undefined {
  try {
    return frontmatterName(readFileSync(path, "utf8"));
  } catch {
    return undefined;
  }
}

/** Each skill name the host would load, mapped to the first file that defines it. */
export function existingSkillNames(scope: SkillScanScope): Map<string, string> {
  const home = scope.env.OPENCODE_TEST_HOME ?? nonEmpty(scope.env.HOME) ?? homedir();
  const roots = [
    ...externalRoots(scope, home),
    ...configRoots(scope, home),
    ...configuredRoots(scope, home),
  ];
  const names = new Map<string, string>();
  for (const root of roots) {
    const files: string[] = [];
    skillFiles(root.dir, root.dot, new Set(), files);
    for (const file of files) {
      const name = readName(file);
      if (name !== undefined && !names.has(name)) names.set(name, file);
    }
  }
  return names;
}
