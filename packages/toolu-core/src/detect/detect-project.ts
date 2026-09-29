/**
 * Project detection (#254): ports of `detect_project_root`, the marker probes
 * (`detect_node_pm`, `detect_rust`, `detect_python`, `detect_ts`), the linter
 * probes (`detect_ts_linter`, `detect_python_linter`, `detect_clippy`) and
 * `to_relative_path`. Every probe looks at the git toplevel of `cwd`, as the
 * bash functions do; outside a repository each answers "none".
 */
import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { basename, join } from "node:path";
import { childEnv, type HostEnv } from "../host/host-name.ts";
import { gitToplevel } from "../host/host-roots.ts";
import { eachLine, isRegularFile } from "./detect-read.ts";

export interface DetectOptions {
  /** Default `process.env`. */
  env?: HostEnv;
  /** Default the process cwd. */
  cwd?: string;
}

export type NodePackageManager = "bun" | "pnpm" | "yarn" | "npm";
export type TsLinter = "biome" | "oxc" | "eslint";

/** `detect_project_root`: the git toplevel of `cwd`, or `undefined` outside a repository. */
export function projectToplevel(options: DetectOptions = {}): string | undefined {
  return gitToplevel(options.env ?? process.env, options.cwd);
}

/** `detect_project_name`: the toplevel's basename. */
export function projectName(options: DetectOptions = {}): string | undefined {
  const root = projectToplevel(options);
  return root === undefined ? undefined : basename(root);
}

function hasAny(root: string, names: readonly string[]): boolean {
  return names.some((name) => isRegularFile(join(root, name)));
}

const LOCK_FILES: readonly (readonly [string, NodePackageManager])[] = [
  ["bun.lock", "bun"],
  ["bun.lockb", "bun"],
  ["pnpm-lock.yaml", "pnpm"],
  ["yarn.lock", "yarn"],
  ["package-lock.json", "npm"],
];

/** `detect_node_pm`: the first lock file present, in bash's order. */
export function nodePackageManager(options: DetectOptions = {}): NodePackageManager | undefined {
  const root = projectToplevel(options);
  if (root === undefined) return undefined;
  return LOCK_FILES.find(([file]) => isRegularFile(join(root, file)))?.[1];
}

/** `detect_rust`: a `Cargo.toml` at the root. */
export function detectRust(options: DetectOptions = {}): boolean {
  const root = projectToplevel(options);
  return root !== undefined && hasAny(root, ["Cargo.toml"]);
}

/** `detect_python`: any of the four Python markers at the root. */
export function detectPython(options: DetectOptions = {}): boolean {
  const root = projectToplevel(options);
  return (
    root !== undefined &&
    hasAny(root, ["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"])
  );
}

/**
 * `detect_ts`: git tracks a `tsconfig*.json`. The same two pathspecs go to
 * `git ls-files`, so git's own pathspec rules (a `*` also matches `/`) decide.
 */
export function detectTs(options: DetectOptions = {}): boolean {
  const env = options.env ?? process.env;
  const root = gitToplevel(env, options.cwd);
  if (root === undefined) return false;
  const res = spawnSync("git", ["-C", root, "ls-files", "**/tsconfig*.json", "tsconfig*.json"], {
    env: childEnv(env),
    encoding: "utf8",
  });
  return res.error === undefined && /[^\n]/.test(res.stdout);
}

/** Root entry names, or none when the directory cannot be listed. */
function entries(root: string): string[] {
  try {
    return readdirSync(root);
  } catch {
    return [];
  }
}

/** `detect_ts_linter`: biome, then oxc, then eslint, by config file at the root. */
export function tsLinter(options: DetectOptions = {}): TsLinter | undefined {
  const root = projectToplevel(options);
  if (root === undefined) return undefined;
  if (hasAny(root, ["biome.json", "biome.jsonc"])) return "biome";
  if (hasAny(root, [".oxlintrc.json"])) return "oxc";
  // `compgen -G`: any entry, file or directory, whose name matches the glob.
  const eslint = entries(root).some(
    (name) => name.startsWith(".eslintrc") || name.startsWith("eslint.config."),
  );
  return eslint ? "eslint" : undefined;
}

/** `detect_python_linter`: a ruff config file, or a `[tool.ruff` line in `pyproject.toml`. */
export function pythonLinter(options: DetectOptions = {}): "ruff" | undefined {
  const root = projectToplevel(options);
  if (root === undefined) return undefined;
  if (hasAny(root, ["ruff.toml", ".ruff.toml"])) return "ruff";
  const pyproject = join(root, "pyproject.toml");
  const walk = isRegularFile(pyproject)
    ? eachLine(pyproject, (line) => line.startsWith("[tool.ruff"))
    : "done";
  return walk === "stopped" ? "ruff" : undefined;
}

/** `detect_clippy`: a clippy config at the root. */
export function detectClippy(options: DetectOptions = {}): boolean {
  const root = projectToplevel(options);
  return root !== undefined && hasAny(root, ["clippy.toml", ".clippy.toml"]);
}

/** `to_relative_path`: `path` relative to the toplevel when under it, else unchanged. */
export function toRelativePath(path: string, options: DetectOptions = {}): string {
  if (path === "") return "";
  const root = projectToplevel(options);
  return root !== undefined && path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path;
}
