/**
 * Real-subprocess runner (#251). `run` is the one spawn primitive: explicit env
 * (never a mutated process.env), stdin written then closed, stdout/stderr/exit
 * captured with wall-clock duration, and a timeout that kills the whole process
 * group so a hook's grandchildren cannot hold the pipes open.
 */
import { existsSync } from "node:fs";
import { basename, extname, resolve } from "node:path";
import { pluginName, resolveEntryCommand } from "./entry-command.ts";
import type { HostName, Sandbox } from "./sandbox.ts";

export type RunResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
};

/** Env overrides; `undefined` unsets the key in the child. */
export type EnvPatch = Record<string, string | undefined>;

export type RunOptions = {
  cwd?: string;
  env?: EnvPatch;
  stdin?: string;
  timeoutMs?: number;
};

const DEFAULT_TIMEOUT_MS = 30_000;
const SIGNAL_EXIT_BASE = 128;
const SIGKILL = 9;

/** Host-session variables that must never leak from the developer's own agent into a spawn. */
const HOST_ENV_PREFIXES = ["CLAUDE_", "CODEX_", "TOOLU_", "OPENCODE_", "CURSOR_"];
const HOST_ENV_KEYS = new Set(["PLUGIN_ROOT"]);

function isHostKey(key: string): boolean {
  return HOST_ENV_KEYS.has(key) || HOST_ENV_PREFIXES.some((prefix) => key.startsWith(prefix));
}

/** `parent` (default process.env) minus host-session keys, then the patch applied. */
export function childEnv(
  patch: EnvPatch = {},
  parent: EnvPatch = process.env,
): Record<string, string> {
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(parent)) {
    if (value !== undefined && !isHostKey(key)) {
      env[key] = value;
    }
  }
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) {
      delete env[key];
    } else {
      env[key] = value;
    }
  }
  return env;
}

function spawnOrMissing(
  argv: string[],
  opts: RunOptions,
): Bun.Subprocess<"pipe", "pipe", "pipe"> | null {
  const cwd = opts.cwd ?? process.cwd();
  // Bun reports a missing cwd as ENOENT too; name it so it is not read as a missing binary.
  if (!existsSync(cwd)) {
    throw new Error(`run: cwd does not exist: ${cwd}`);
  }
  try {
    return Bun.spawn(argv, {
      cwd,
      env: childEnv(opts.env),
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
      detached: true,
    });
  } catch (err: unknown) {
    if (isErrno(err, "ENOENT")) {
      return null;
    }
    throw err;
  }
}

function killGroup(pid: number): void {
  try {
    process.kill(-pid, "SIGKILL");
  } catch (err: unknown) {
    // ESRCH: the group exited between the timer firing and the kill.
    if (!isErrno(err, "ESRCH")) {
      throw err;
    }
  }
}

function isErrno(err: unknown, code: string): boolean {
  return err instanceof Error && "code" in err && err.code === code;
}

/** Write and close stdin. EPIPE means the child exited without reading it; its exit status still tells the story. */
async function feedStdin(
  proc: Bun.Subprocess<"pipe", "pipe", "pipe">,
  body: string,
): Promise<void> {
  try {
    await proc.stdin.write(body);
    await proc.stdin.end();
  } catch (err: unknown) {
    if (!isErrno(err, "EPIPE")) {
      throw err;
    }
  }
}

/** Spawn `argv` and resolve its captured result. Never rejects for a nonzero exit. */
export async function run(argv: string[], opts: RunOptions = {}): Promise<RunResult> {
  const [command = ""] = argv;
  const started = performance.now();
  const proc = spawnOrMissing(argv, opts);
  if (proc === null) {
    return {
      exitCode: 127,
      stdout: "",
      stderr: `${command}: command not found`,
      durationMs: 0,
      timedOut: false,
    };
  }
  let timedOut = false;
  let killError: unknown = null;
  const timer = setTimeout(() => {
    timedOut = true;
    try {
      killGroup(proc.pid);
    } catch (err: unknown) {
      // A throw inside a timer would be uncaught: stop the direct child instead
      // and surface the group-kill failure to the caller once it exits.
      killError = err;
      proc.kill("SIGKILL");
    }
  }, opts.timeoutMs ?? DEFAULT_TIMEOUT_MS);
  await feedStdin(proc, opts.stdin ?? "");
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  clearTimeout(timer);
  if (killError !== null) {
    throw killError;
  }
  const exitCode = proc.exitCode ?? SIGNAL_EXIT_BASE + SIGKILL;
  return { exitCode, stdout, stderr, durationMs: performance.now() - started, timedOut };
}

const HOST_ENV: Record<HostName, (sandbox: Sandbox, pluginRoot: string) => Record<string, string>> =
  {
    claude: (sandbox, pluginRoot) => ({
      CLAUDE_PLUGIN_ROOT: pluginRoot,
      CLAUDE_PROJECT_DIR: sandbox.project,
      CLAUDE_CONFIG_DIR: sandbox.configDir("claude", "user"),
    }),
    codex: (sandbox, pluginRoot) => ({ PLUGIN_ROOT: pluginRoot, CODEX_HOME: sandbox.codexHome }),
    cursor: (sandbox) => ({ CURSOR_PROJECT_DIR: sandbox.project }),
    opencode: () => ({ TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode" }),
  };

/** The environment a host puts on a hook process, rooted in the sandbox. */
export function hostEnv(
  host: HostName,
  sandbox: Sandbox,
  pluginRoot: string,
): Record<string, string> {
  return { HOME: sandbox.home, TOOLU_HOST_OVERRIDE: host, ...HOST_ENV[host](sandbox, pluginRoot) };
}

export type RunHookOptions = {
  host: HostName;
  sandbox: Sandbox;
  pluginRoot: string;
  /** The plugin `TOOLU_IMPL` names; defaults to `pluginRoot`'s plugin.json name. */
  plugin?: string;
  /** A committed bundle, run as `bun <bundle>` or as its selected Rust command. */
  bundle?: string;
  /** An explicit command, for bash hooks during the strangler migration. */
  argv?: string[];
  /** Hook stdin: a string is passed through, anything else is JSON-encoded. */
  stdin: unknown;
  env?: EnvPatch;
  timeoutMs?: number;
};

function hookArgv(opts: RunHookOptions): string[] {
  if (opts.bundle !== undefined && opts.argv !== undefined) {
    throw new Error("runHook: pass bundle or argv, not both");
  }
  if (opts.bundle !== undefined) {
    const { argv, implementation } = resolveEntryCommand({
      plugin: opts.plugin ?? pluginName(opts.pluginRoot),
      entry: basename(opts.bundle, extname(opts.bundle)),
      bundle: resolve(opts.bundle),
      defaultArgv: [process.execPath, opts.bundle],
    });
    if (implementation === "bun" && !existsSync(opts.bundle)) {
      throw new Error(`bundle not found: ${opts.bundle}`);
    }
    return argv;
  }
  if (opts.argv === undefined || opts.argv.length === 0) {
    throw new Error("runHook: pass bundle or argv");
  }
  return opts.argv;
}

/** Spawn a hook the way `host` would, with cwd at the sandbox project. */
export function runHook(opts: RunHookOptions): Promise<RunResult> {
  const argv = hookArgv(opts);
  const stdin = typeof opts.stdin === "string" ? opts.stdin : JSON.stringify(opts.stdin);
  const runOpts: RunOptions = {
    cwd: opts.sandbox.project,
    env: { ...hostEnv(opts.host, opts.sandbox, opts.pluginRoot), ...opts.env },
    stdin,
  };
  if (opts.timeoutMs !== undefined) {
    runOpts.timeoutMs = opts.timeoutMs;
  }
  return run(argv, runOpts);
}
