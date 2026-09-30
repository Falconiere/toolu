/**
 * Host-neutral project status shared by the Claude renderer and the Codex
 * report: repository, branch, ahead/behind, working-tree counts, the quality
 * gate, the comemory memory count and Jev readiness, collected in-process.
 *
 * Jev readiness is local configuration only: the wrapper is never executed and
 * no request is sent on the statusline's per-prompt hot path.
 */
import { accessSync, constants, lstatSync, statSync, type Stats } from "node:fs";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { configRoot, envValue, projectStateRoot, type HostEnv } from "@toolu/core/host";
import { readObject } from "./json.ts";

export type StatusHost = "claude" | "codex";

export type ProjectStatus = {
  host: StatusHost;
  cwd: string;
  repo_root: string;
  folder: string;
  branch: string;
  ahead: number;
  behind: number;
  working_tree: { staged: number; unstaged: number; untracked: number };
  gate: { status: string; reason: string };
  jev: { status: "" | "ready" | "unavailable"; reason: string };
  comemory_count: number | null;
};

/** Codex when forced, or when unforced and Codex's `PLUGIN_ROOT` is set; otherwise Claude. */
export function statusHost(env: HostEnv): StatusHost {
  const override = envValue(env, "TOOLU_HOST_OVERRIDE");
  if (override === "codex") return "codex";
  return override === undefined && envValue(env, "PLUGIN_ROOT") !== undefined ? "codex" : "claude";
}

/** The status with no project and no Jev: what a renderer shows when collection fails. */
export function emptyStatus(host: StatusHost, cwd: string): ProjectStatus {
  return {
    host,
    cwd,
    repo_root: "",
    folder: "",
    branch: "",
    ahead: 0,
    behind: 0,
    working_tree: { staged: 0, unstaged: 0, untracked: 0 },
    gate: { status: "", reason: "" },
    jev: { status: "", reason: "" },
    comemory_count: null,
  };
}

function childEnv(env: HostEnv): Record<string, string> {
  return Object.fromEntries(
    Object.entries(env).filter((entry): entry is [string, string] => entry[1] !== undefined),
  );
}

/** stdout of `git -C cwd --no-optional-locks …` with trailing newlines trimmed; undefined on failure. */
function git(cwd: string, env: HostEnv, args: readonly string[]): string | undefined {
  try {
    const res = Bun.spawnSync(["git", "-C", cwd, "--no-optional-locks", ...args], {
      env: childEnv(env),
      stdout: "pipe",
      stderr: "ignore",
    });
    return res.exitCode === 0 ? res.stdout.toString().replace(/\n+$/, "") : undefined;
  } catch {
    return undefined;
  }
}

/** A stat that answers undefined on any failure (ENOENT, ENOTDIR, ELOOP…), as `test -d/-f/-L` answers false. */
function probe(read: () => Stats): Stats | undefined {
  try {
    return read();
  } catch {
    return undefined;
  }
}

function isDirectory(path: string): boolean {
  return path !== "" && (probe(() => statSync(path))?.isDirectory() ?? false);
}

function isFile(path: string): boolean {
  return probe(() => statSync(path))?.isFile() ?? false;
}

/** `basename` as coreutils prints it: `/` stays `/`. */
function folderName(cwd: string): string {
  const name = basename(cwd);
  return name === "" && cwd.startsWith("/") ? "/" : name;
}

function stringMember(doc: Record<string, unknown> | undefined, key: string): string {
  const value = doc?.[key];
  return typeof value === "string" ? value : "";
}

type TreeCounts = Pick<ProjectStatus, "ahead" | "behind" | "working_tree">;

/** `## branch...upstream [ahead N, behind M]`: counts only from the trailing bracket. */
function aheadBehind(header: string): { ahead: number; behind: number } {
  const bracket = /\[([^\]]*)\]$/.exec(header)?.[1] ?? "";
  const count = (word: string): number =>
    Number(new RegExp(`(?:^|, )${word} (\\d+)`).exec(bracket)?.[1] ?? 0);
  return { ahead: count("ahead"), behind: count("behind") };
}

/** `git status --porcelain --branch`: the header, then one XY line per path. */
function treeCounts(cwd: string, env: HostEnv): TreeCounts {
  const lines = (git(cwd, env, ["status", "--porcelain", "--branch"]) ?? "").split("\n");
  const [header = "", ...entries] = lines;
  const counts = { staged: 0, unstaged: 0, untracked: 0 };
  for (const line of entries) {
    if (line.length < 2) continue;
    const xy = line.slice(0, 2);
    if (xy === "??") counts.untracked += 1;
    else if (xy !== "!!") {
      if (xy[0] !== " ") counts.staged += 1;
      if (xy[1] !== " ") counts.unstaged += 1;
    }
  }
  const ab = header.startsWith("## ") ? aheadBehind(header) : { ahead: 0, behind: 0 };
  return { ...ab, working_tree: counts };
}

/** `<config root>/comemory-status/<main repo basename>.json` `count`, keyed through the common dir so a worktree shares its main checkout's count. */
function comemoryCount(cwd: string, commonDir: string, root: string): number | null {
  if (commonDir === "") return null;
  const abs = isAbsolute(commonDir) ? commonDir : resolve(cwd, commonDir);
  const marker = readObject(join(root, "comemory-status", `${basename(dirname(abs))}.json`));
  const count = marker?.["count"];
  return typeof count === "number" ? count : null;
}

function executableFile(path: string): boolean {
  if (!isFile(path)) return false;
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function jevReadiness(env: HostEnv, root: string): ProjectStatus["jev"] {
  const wrapper = join(root, "jev", "jev.sh");
  if (probe(() => lstatSync(wrapper)) === undefined) return { status: "", reason: "" };
  const reasons: string[] = [];
  if (!executableFile(wrapper)) reasons.push("missing executable wrapper");
  if (Bun.which("curl", { PATH: envValue(env, "PATH") ?? "" }) === null) {
    reasons.push("missing curl");
  }
  const key = envValue(env, "TYPESAFE_API_KEY");
  if (key === undefined) reasons.push("missing TYPESAFE_API_KEY");
  else if (/[\r\n]/.test(key)) reasons.push("invalid TYPESAFE_API_KEY");
  return reasons.length === 0
    ? { status: "ready", reason: "" }
    : { status: "unavailable", reason: reasons.join("; ") };
}

/** Status for `cwd`; an empty `cwd` is profile-only (no project fields, Jev still evaluated). */
export function collectStatus(cwd: string, env: HostEnv, host: StatusHost): ProjectStatus {
  const status = emptyStatus(host, cwd);
  const root = configRoot({ env, host });
  let commonDir = "";
  if (isDirectory(cwd)) {
    status.folder = folderName(cwd);
    const [top = "", common = ""] = (
      git(cwd, env, ["rev-parse", "--show-toplevel", "--git-common-dir"]) ?? ""
    ).split("\n");
    status.repo_root = top;
    commonDir = common;
  }
  if (status.repo_root !== "") {
    status.branch = git(cwd, env, ["symbolic-ref", "--short", "HEAD"]) ?? "";
    Object.assign(status, treeCounts(cwd, env));
    status.comemory_count = comemoryCount(cwd, commonDir, root);
  }
  if (cwd !== "") {
    const stateRoot = projectStateRoot({ env, host, root: status.repo_root || cwd });
    const gate =
      stateRoot === undefined ? undefined : readObject(join(stateRoot, "quality-gate-status.json"));
    status.gate = { status: stringMember(gate, "status"), reason: stringMember(gate, "reason") };
  }
  status.jev = jevReadiness(env, root);
  return status;
}
