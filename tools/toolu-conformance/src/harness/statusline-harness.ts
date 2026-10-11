/** Shared fixtures for the statusline suites: real git repos, real bundles, a per-test sandbox. */
import { expect } from "bun:test";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { bundlePath, entryArgv, pluginRoot } from "@toolu/conformance/harness/entry-command";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";

const PLUGIN = resolve(import.meta.dir, "../../../../plugins/statusline");
const REPO = resolve(PLUGIN, "../..");
const RUST =
  process.env.TOOLU_IMPL === "rust" || (process.env.TOOLU_IMPL?.startsWith("rust:") ?? false);
export const BINARY = join(REPO, "target", RUST ? "release" : "debug", "toolu");
const JEV_ROOT = pluginRoot("jev");
const jevBundle = bundlePath(JEV_ROOT, "jev");
/** The published helper source: the Bun bundle while it exists, otherwise the native shim. */
export const JEV_BUNDLE = existsSync(jevBundle) ? jevBundle : join(JEV_ROOT, "scripts/jev.sh");

function jevSessionArgv(): string[] {
  const bundle = bundlePath(JEV_ROOT, "session-start");
  if (existsSync(bundle)) return entryArgv("jev", "session-start", JEV_ROOT);
  return [
    BINARY,
    "jev",
    "hook",
    "session-start",
    "--event",
    "SessionStart",
    "--plugin-root",
    JEV_ROOT,
  ];
}

export const KEY = "statusline-test-key";

/** Preserve fixture action order without overlapping writes to one sandbox. */
export function runInOrder<T>(
  items: readonly T[],
  task: (item: T) => Promise<void>,
): Promise<void> {
  return items.reduce((pending, item) => pending.then(() => task(item)), Promise.resolve());
}

export function put(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

export function git(cwd: string, ...args: string[]): void {
  const res = Bun.spawnSync(
    ["git", "-C", cwd, "-c", "user.email=t@t", "-c", "user.name=t", ...args],
    { stdout: "ignore", stderr: "pipe" },
  );
  if (res.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr.toString()}`);
}

/** A repo with one empty commit on `main`. */
export function repo(dir: string): string {
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "-q", "-b", "main");
  git(dir, "commit", "-q", "--allow-empty", "-m", "init");
  return dir;
}

/** `<root>/repo` on `branch`, pushed to and tracking a bare `<root>/remote.git`. */
export function withRemote(sb: Sandbox, branch = "main"): string {
  const remote = join(sb.root, "remote.git");
  const dir = join(sb.root, "repo");
  mkdirSync(remote, { recursive: true });
  git(remote, "init", "-q", "--bare");
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "-q", "-b", branch);
  git(dir, "commit", "-q", "--allow-empty", "-m", "init");
  git(dir, "remote", "add", "origin", remote);
  git(dir, "push", "-q", "-u", "origin", branch);
  return dir;
}

/** The Claude config dir every renderer run uses. */
export function cfgOf(sb: Sandbox): string {
  return join(sb.root, "cfg");
}

/** Publish the Jev wrapper into `root` through Jev's real SessionStart bundle. */
export async function publishJev(
  root: string,
  host: "claude" | "codex",
  extra: EnvPatch = {},
): Promise<void> {
  const env =
    host === "claude"
      ? { CLAUDE_CONFIG_DIR: root, TOOLU_HOST_OVERRIDE: "claude", ...extra }
      : { CODEX_HOME: root, TOOLU_HOST_OVERRIDE: "codex", ...extra };
  const res = await run(jevSessionArgv(), {
    env,
    stdin: "",
  });
  expect(res.exitCode).toBe(0);
}

export const payload = (cwd: string, extra = ""): string =>
  `{"model":{"display_name":"Opus"},"workspace":{"current_dir":${JSON.stringify(cwd)}},"context_window":{"context_window_size":200000,"total_input_tokens":1000}${extra}}`;

type RenderOpts = { payload?: string; cwd?: string; env?: EnvPatch };

function workspaceDir(body: string, fallback: string): string {
  try {
    const parsed: unknown = JSON.parse(body);
    if (typeof parsed !== "object" || parsed === null || !("workspace" in parsed)) return fallback;
    const workspace = parsed.workspace;
    if (typeof workspace !== "object" || workspace === null || !("current_dir" in workspace))
      return fallback;
    return typeof workspace.current_dir === "string" ? workspace.current_dir : fallback;
  } catch {
    return fallback;
  }
}

/** Run the renderer the way Claude Code does; the payload defaults to `payload(sb.project)`. */
export async function render(sb: Sandbox, opts: RenderOpts = {}): Promise<string> {
  const env = {
    HOME: sb.home,
    CLAUDE_CONFIG_DIR: cfgOf(sb),
    TOOLU_HOST_OVERRIDE: "claude",
    TYPESAFE_API_KEY: KEY,
    PATH: `${dirname(BINARY)}:${process.env.PATH ?? ""}`,
    ...opts.env,
  };
  const body = opts.payload ?? payload(sb.project);
  const worktree = workspaceDir(body, sb.project);
  const gitRoot = Bun.spawnSync(["git", "-C", worktree, "rev-parse", "--show-toplevel"], {
    stdout: "pipe",
    stderr: "ignore",
  });
  if (gitRoot.exitCode === 0) {
    const primed = await run([BINARY, "statusline", "refresh-cache", worktree], {
      cwd: opts.cwd ?? sb.project,
      env,
      stdin: "",
    });
    expect(primed).toMatchObject({ exitCode: 0, stderr: "" });
  }
  const res = await run([BINARY, "statusline", "render"], {
    cwd: opts.cwd ?? sb.project,
    env,
    stdin: body,
  });
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  return res.stdout;
}

/** Strip ANSI colour codes. */
export function plain(text: string): string {
  return text.replaceAll(new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, "g"), "");
}
