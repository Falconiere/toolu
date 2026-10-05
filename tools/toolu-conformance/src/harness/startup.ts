/**
 * SessionStart startup hooks, run the way a host runs them (#269): the
 * plugin's real `hooks.json` launcher command under `sh -c`, with the host's
 * environment rooted in a sandbox. `publishedCliSuite` registers the cases
 * every plugin that publishes a Bun CLI at a stable config-root path shares.
 */
import { expect, test } from "bun:test";
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { z } from "zod";
import { bundlePath, pluginName, resolveEntryCommand } from "./entry-command.ts";
import { createSandbox, type Sandbox } from "./sandbox.ts";
import { run, type EnvPatch, type RunResult } from "./spawn.ts";

const HooksFileSchema = z.object({
  hooks: z.record(
    z.string(),
    z.array(z.object({ hooks: z.array(z.object({ command: z.string() })) })),
  ),
});

/** The `hooks.json` command under `event` that launches `hooks/dist/<entry>.js`. */
export function hookCommand(pluginRoot: string, event: string, entry: string): string {
  const file = HooksFileSchema.parse(
    JSON.parse(readFileSync(join(pluginRoot, "hooks/hooks.json"), "utf8")),
  );
  // The launcher quotes the whole bundle argument (launcherCommand in
  // @toolu/core/launcher), so this exact quoted text appears literally in the
  // command; the closing quote keeps `check` from matching `check-deps.js`.
  const bundle = `"\${CLAUDE_PLUGIN_ROOT}/hooks/dist/${entry}.js"`;
  const commands = (file.hooks[event] ?? []).flatMap((group) => group.hooks);
  const found = commands.find((hook) => hook.command.includes(bundle));
  if (found === undefined) throw new Error(`no ${event} hook launches ${entry} in ${pluginRoot}`);
  return found.command;
}

export type StartupHost = "claude" | "codex";

/** The environment Claude Code or Codex gives a plugin hook, rooted in `sb`. */
export function startupEnv(host: StartupHost, sb: Sandbox, pluginRoot: string): EnvPatch {
  return host === "claude"
    ? {
        HOME: sb.home,
        CLAUDE_PLUGIN_ROOT: pluginRoot,
        CLAUDE_PROJECT_DIR: sb.project,
        CLAUDE_CONFIG_DIR: join(sb.home, ".claude"),
      }
    : {
        HOME: sb.home,
        CLAUDE_PLUGIN_ROOT: pluginRoot,
        PLUGIN_ROOT: pluginRoot,
        CODEX_HOME: sb.codexHome,
      };
}

/** The config root `host` publishes under for `startupEnv`. */
export function startupRoot(host: StartupHost, sb: Sandbox): string {
  return host === "claude" ? join(sb.home, ".claude") : sb.codexHome;
}

/**
 * Run `pluginRoot`'s SessionStart launcher for `entry` with `env`, stdin naming
 * `source`; under `TOOLU_IMPL`, the selected Rust command runs instead.
 */
export function runStartupHook(
  pluginRoot: string,
  entry: string,
  sb: Sandbox,
  env: EnvPatch,
  source = "startup",
): Promise<RunResult> {
  const { argv } = resolveEntryCommand({
    plugin: pluginName(pluginRoot),
    entry,
    bundle: resolve(bundlePath(pluginRoot, entry)),
    defaultArgv: ["sh", "-c", hookCommand(pluginRoot, "SessionStart", entry)],
  });
  return run(argv, { cwd: sb.project, env, stdin: JSON.stringify({ source }) });
}

export type PublishedCliSpec = {
  plugin: string;
  pluginRoot: string;
  /** Plugin-relative path of the published file, e.g. `hooks/dist/search.js`. */
  source: string;
  dir: string;
  name: string;
  /** Exact stderr line when `bun` is not on PATH. */
  advisory: string;
  /** Credential variables the plugin's CLI reads; the hook must not care. */
  credentials: Record<string, string>;
  /** Running the published path with `args` (and `env`) exits `exitCode` and prints `output`. */
  probe: { args: string[]; env?: EnvPatch; exitCode: number; output: string };
  /** The exact `systemMessage` a deprecated plugin prints on each host (#403); absent: silent. */
  notice?: Record<StartupHost | "opencode", string>;
};

/** The hook's exact stdout on `host`: the notice object, or nothing. */
function noticeStdout(spec: PublishedCliSpec, host: StartupHost): string {
  const notice = spec.notice?.[host];
  return notice === undefined ? "" : `${JSON.stringify({ systemMessage: notice })}\n`;
}

/** `credentials` set, or each one explicitly unset. */
function withCredentials(spec: PublishedCliSpec, set: boolean): EnvPatch {
  return Object.fromEntries(
    Object.entries(spec.credentials).map(([key, value]) => [key, set ? value : undefined]),
  );
}

/** One publish under `host`, credentials set or unset; returns the hook's output streams. */
async function publishOnce(spec: PublishedCliSpec, host: StartupHost, set: boolean) {
  using sb = createSandbox();
  const env = { ...startupEnv(host, sb, spec.pluginRoot), ...withCredentials(spec, set) };
  const res = await runStartupHook(spec.pluginRoot, "session-start", sb, env);
  expect(res).toMatchObject({ exitCode: 0, stdout: noticeStdout(spec, host), stderr: "" });
  const dst = join(startupRoot(host, sb), spec.dir, spec.name);
  expect(readlinkSync(dst)).toBe(join(spec.pluginRoot, spec.source));
  const probeEnv = { ...env, ...spec.probe.env };
  const probe = await run([dst, ...spec.probe.args], { cwd: sb.project, env: probeEnv });
  expect(probe.exitCode).toBe(spec.probe.exitCode);
  expect(probe.stdout + probe.stderr).toContain(spec.probe.output);
  return [res.stdout, res.stderr];
}

async function publishes(spec: PublishedCliSpec, host: StartupHost): Promise<void> {
  const [withCreds, without] = await Promise.all([
    publishOnce(spec, host, true),
    publishOnce(spec, host, false),
  ]);
  expect(withCreds).toEqual(without);
}

async function refreshesAndKeeps(spec: PublishedCliSpec): Promise<void> {
  using sb = createSandbox();
  const env = startupEnv("claude", sb, spec.pluginRoot);
  const dir = join(startupRoot("claude", sb), spec.dir);
  mkdirSync(dir, { recursive: true });
  symlinkSync(`/nonexistent/old/${spec.name}`, join(dir, spec.name));
  writeFileSync(join(dir, "keep"), "");
  await runStartupHook(spec.pluginRoot, "session-start", sb, env);
  const first = lstatSync(join(dir, spec.name));
  expect(readlinkSync(join(dir, spec.name))).toBe(join(spec.pluginRoot, spec.source));
  await runStartupHook(spec.pluginRoot, "session-start", sb, env);
  expect(lstatSync(join(dir, spec.name)).ino).toBe(first.ino);

  using user = createSandbox();
  const userDir = join(startupRoot("claude", user), spec.dir);
  mkdirSync(userDir, { recursive: true });
  writeFileSync(join(userDir, spec.name), "#!/usr/bin/env bash\necho user-override\n");
  const res = await runStartupHook(spec.pluginRoot, "session-start", user, {
    ...startupEnv("claude", user, spec.pluginRoot),
  });
  expect(res.exitCode).toBe(0);
  expect(lstatSync(join(userDir, spec.name)).isSymbolicLink()).toBe(false);
  expect(readFileSync(join(userDir, spec.name), "utf8")).toBe(
    "#!/usr/bin/env bash\necho user-override\n",
  );
}

async function advisesWithoutBunOnPath(spec: PublishedCliSpec): Promise<void> {
  using sb = createSandbox();
  const env = {
    ...startupEnv("claude", sb, spec.pluginRoot),
    PATH: "/usr/bin:/bin",
    TOOLU_BUN: process.execPath,
  };
  const res = await runStartupHook(spec.pluginRoot, "session-start", sb, env);
  expect(res).toMatchObject({
    exitCode: 0,
    stdout: noticeStdout(spec, "claude"),
    stderr: `${spec.advisory}\n`,
  });
  expect(existsSync(join(startupRoot("claude", sb), spec.dir, spec.name))).toBe(true);
}

async function noBunAtAll(spec: PublishedCliSpec): Promise<void> {
  using sb = createSandbox();
  const env = { ...startupEnv("claude", sb, spec.pluginRoot), PATH: "/usr/bin:/bin" };
  const res = await runStartupHook(spec.pluginRoot, "session-start", sb, env);
  expect(res.exitCode).toBe(0);
  const advisory = z.object({ systemMessage: z.string() }).parse(JSON.parse(res.stdout));
  expect(advisory.systemMessage).toStartWith(`${spec.plugin} plugin: Bun runtime not found`);
  expect(existsSync(join(startupRoot("claude", sb), spec.dir))).toBe(false);
}

async function failsSoftWithoutSource(spec: PublishedCliSpec): Promise<void> {
  using sb = createSandbox();
  const fake = join(sb.root, "fake-plugin");
  const bundle = bundlePath(fake, "session-start");
  mkdirSync(dirname(bundle), { recursive: true });
  copyFileSync(bundlePath(spec.pluginRoot, "session-start"), bundle);
  const env = startupEnv("claude", sb, fake);
  const { argv } = resolveEntryCommand({ plugin: spec.plugin, entry: "session-start", bundle });
  const res = await run(argv, { cwd: sb.project, env, stdin: "{}" });
  expect(res).toMatchObject({ exitCode: 0, stdout: noticeStdout(spec, "claude"), stderr: "" });
  expect(existsSync(join(startupRoot("claude", sb), spec.dir))).toBe(false);
}

const NoticeOutput = z.strictObject({
  hookSpecificOutput: z
    .strictObject({ hookEventName: z.literal("SessionStart"), additionalContext: z.string() })
    .optional(),
  systemMessage: z.string().optional(),
});

/**
 * On OpenCode the notice rides beside any startup context at a start, and is
 * left out at a compaction, which OpenCode runs SessionStart again for (#403).
 */
async function opencodeNotice(spec: PublishedCliSpec, notice: string): Promise<void> {
  using sb = createSandbox();
  const env = {
    HOME: sb.home,
    CLAUDE_PLUGIN_ROOT: spec.pluginRoot,
    TOOLU_BUN: process.execPath,
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: join(sb.project, ".opencode/toolu/state"),
  };
  // In order, as OpenCode runs them: both publish the same helper path.
  const start = await runStartupHook(spec.pluginRoot, "session-start", sb, env, "startup");
  expect(start).toMatchObject({ exitCode: 0, stderr: "" });
  const started = NoticeOutput.parse(JSON.parse(start.stdout));
  expect(started.systemMessage).toBe(notice);
  expect(started.hookSpecificOutput?.additionalContext ?? "").not.toContain("deprecated");
  const compact = await runStartupHook(spec.pluginRoot, "session-start", sb, env, "compact");
  expect(compact).toMatchObject({ exitCode: 0, stderr: "" });
  const compacted = compact.stdout === "" ? {} : NoticeOutput.parse(JSON.parse(compact.stdout));
  expect(compacted.systemMessage).toBeUndefined();
}

/** Register the shared published-CLI SessionStart cases for one plugin. */
export function publishedCliSuite(spec: PublishedCliSpec): void {
  const name = `${spec.plugin} session-start`;
  test.concurrent(`${name}: publishes ${spec.dir}/${spec.name} on Claude, creds or not`, () =>
    publishes(spec, "claude"));
  test.concurrent(`${name}: publishes ${spec.dir}/${spec.name} on Codex, creds or not`, () =>
    publishes(spec, "codex"));
  test.concurrent(`${name}: refreshes a stale link, is idempotent, keeps a user file`, () =>
    refreshesAndKeeps(spec));
  test.concurrent(`${name}: bun off PATH -> one advisory line, still publishes`, () =>
    advisesWithoutBunOnPath(spec));
  test.concurrent(`${name}: no Bun at all -> launcher advisory, nothing published`, () =>
    noBunAtAll(spec));
  const quiet = spec.notice === undefined ? "silent" : "only the notice";
  test.concurrent(`${name}: missing CLI bundle -> ${quiet}, nothing published`, () =>
    failsSoftWithoutSource(spec));
  const notice = spec.notice?.opencode;
  if (notice !== undefined) {
    test.concurrent(`${name}: OpenCode notice once at start, none at compaction`, () =>
      opencodeNotice(spec, notice));
  }
}
