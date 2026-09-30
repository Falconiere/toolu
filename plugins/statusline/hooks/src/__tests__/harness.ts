/** Shared fixtures for the statusline suites: real git repos, real bundles, a per-test sandbox. */
import { expect } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";

export const PLUGIN = resolve(import.meta.dir, "../../..");
export const RENDER = join(PLUGIN, "hooks/dist/statusline.js");
export const STATUS = join(PLUGIN, "hooks/dist/status.js");
export const SETUP = join(PLUGIN, "hooks/dist/setup.js");
export const JEV_BUNDLE = resolve(PLUGIN, "../jev/hooks/dist/jev.js");
const JEV_HOOK = resolve(PLUGIN, "../jev/hooks/dist/session-start.js");

export const KEY = "statusline-test-key";

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
  const res = await run([process.execPath, JEV_HOOK], { env, stdin: "" });
  expect(res.exitCode).toBe(0);
}

export const payload = (cwd: string, extra = ""): string =>
  `{"model":{"display_name":"Opus"},"workspace":{"current_dir":${JSON.stringify(cwd)}},"context_window":{"context_window_size":200000,"total_input_tokens":1000}${extra}}`;

type RenderOpts = { payload?: string; cwd?: string; env?: EnvPatch };

/** Run the renderer the way Claude Code does; the payload defaults to `payload(sb.project)`. */
export async function render(sb: Sandbox, opts: RenderOpts = {}): Promise<string> {
  const env = { HOME: sb.home, CLAUDE_CONFIG_DIR: cfgOf(sb), TYPESAFE_API_KEY: KEY, ...opts.env };
  const res = await run([RENDER], {
    cwd: opts.cwd ?? sb.project,
    env,
    stdin: opts.payload ?? payload(sb.project),
  });
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  return res.stdout;
}

/** Run the Codex report for `dir` (the process cwd when omitted). */
export async function report(
  sb: Sandbox,
  dir: string | undefined,
  env: EnvPatch = {},
  cwd = sb.root,
): Promise<string> {
  const argv = dir === undefined ? [process.execPath, STATUS] : [process.execPath, STATUS, dir];
  const res = await run(argv, {
    cwd,
    env: { HOME: sb.home, CODEX_HOME: sb.codexHome, TYPESAFE_API_KEY: KEY, ...env },
  });
  expect(res).toMatchObject({ exitCode: 0, stderr: "" });
  return res.stdout;
}

/** Strip ANSI colour codes. */
export function plain(text: string): string {
  return text.replaceAll(/\x1b\[[0-9;]*m/g, "");
}
