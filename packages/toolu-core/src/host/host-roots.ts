/**
 * Host roots (#252): config, project, state and plugin paths, invocation syntax
 * and install command per host. Port of the path half of `host.sh`; Claude and
 * Codex resolve exactly as the bash functions do.
 */
import { spawnSync } from "node:child_process";
import { homedir } from "node:os";
import { join } from "node:path";
import { detectHost } from "./host-detect.ts";
import { childEnv, envValue, type HostEnv, type HostName } from "./host-name.ts";

export type HostOptions = { env?: HostEnv; host?: HostName };

type ProjectOptions = HostOptions & { cwd?: string };
type StateOptions = ProjectOptions & { root?: string };

/**
 * `options` with env and host filled in (host detected when absent). Functions
 * that call siblings resolve once and pass the result down, so detection (and
 * its invalid-override warning) runs once per call.
 */
export function resolveHost<T extends HostOptions>(
  options: T,
): T & { env: HostEnv; host: HostName } {
  const env = options.env ?? process.env;
  return { ...options, env, host: options.host ?? detectHost({ env }) };
}

function home(env: HostEnv): string {
  return envValue(env, "HOME") ?? homedir();
}

const NATIVE_CONFIG_ROOT: Readonly<Record<HostName, (env: HostEnv) => string>> = {
  claude: (env) => envValue(env, "CLAUDE_CONFIG_DIR") ?? join(home(env), ".claude"),
  codex: (env) => envValue(env, "CODEX_HOME") ?? join(home(env), ".codex"),
  cursor: (env) => join(home(env), ".cursor"),
  hermes: (env) => envValue(env, "HERMES_HOME") ?? join(home(env), ".hermes"),
  opencode: (env) =>
    envValue(env, "TOOLU_OPENCODE_HOME") ??
    join(envValue(env, "XDG_CONFIG_HOME") ?? join(home(env), ".config"), "opencode"),
};

/** Host-native writable config/data root; `TOOLU_CONFIG_DIR` wins everywhere. */
export function configRoot(options: HostOptions = {}): string {
  const { env, host } = resolveHost(options);
  return envValue(env, "TOOLU_CONFIG_DIR") ?? NATIVE_CONFIG_ROOT[host](env);
}

function gitToplevel(env: HostEnv, cwd: string | undefined): string | undefined {
  const res = spawnSync("git", ["rev-parse", "--show-toplevel"], {
    cwd: cwd ?? process.cwd(),
    env: childEnv(env),
    encoding: "utf8",
  });
  if (res.error !== undefined || res.status !== 0) {
    return undefined;
  }
  const top = res.stdout.trim();
  return top === "" ? undefined : top;
}

const PROJECT_DIR_VAR: Readonly<Partial<Record<HostName, string>>> = {
  claude: "CLAUDE_PROJECT_DIR",
  cursor: "CURSOR_PROJECT_DIR",
};

/** `TOOLU_PROJECT_DIR`, the host's own project variable, then the git toplevel of `cwd`. */
export function projectRoot(options: ProjectOptions = {}): string | undefined {
  const { env, host } = resolveHost(options);
  const hostVar = PROJECT_DIR_VAR[host];
  return (
    envValue(env, "TOOLU_PROJECT_DIR") ??
    (hostVar === undefined ? undefined : envValue(env, hostVar)) ??
    gitToplevel(env, options.cwd)
  );
}

/** `.claude`, `.codex`, `.cursor`, `.hermes` or `.opencode`, unless overridden. */
export function projectDirname(options: HostOptions = {}): string {
  const { env, host } = resolveHost(options);
  return envValue(env, "TOOLU_PROJECT_CONFIG_DIRNAME") ?? `.${host}`;
}

/** `<project>/<dirname>/toolu.config.json`, or `undefined` outside a project. */
export function projectConfigPath(options: ProjectOptions = {}): string | undefined {
  const o = resolveHost(options);
  const root = projectRoot(o);
  return root === undefined ? undefined : join(root, projectDirname(o), "toolu.config.json");
}

/** `<root>/<dirname>/tmp`, with `root` defaulting to the project root. */
export function projectStateRoot(options: StateOptions = {}): string | undefined {
  const o = resolveHost(options);
  const root = o.root ?? projectRoot(o);
  return root === undefined ? undefined : join(root, projectDirname(o), "tmp");
}

/** `<state root>/<name>`; an empty name is a caller error. */
export function projectStateDir(name: string, options: StateOptions = {}): string | undefined {
  if (name === "") {
    throw new TypeError("projectStateDir: name must be non-empty");
  }
  const base = projectStateRoot(resolveHost(options));
  return base === undefined ? undefined : join(base, name);
}

const PLUGIN_ROOT_VAR: Readonly<Partial<Record<HostName, string>>> = {
  codex: "PLUGIN_ROOT",
  cursor: "CURSOR_PLUGIN_ROOT",
  opencode: "TOOLU_PLUGIN_ROOT",
};

/** The host's own plugin root variable, then `CLAUDE_PLUGIN_ROOT`. */
export function pluginRoot(options: HostOptions = {}): string | undefined {
  const { env, host } = resolveHost(options);
  const hostVar = PLUGIN_ROOT_VAR[host];
  const own = hostVar === undefined ? undefined : envValue(env, hostVar);
  return own ?? envValue(env, "CLAUDE_PLUGIN_ROOT");
}

/** `PLUGIN_DATA` on Codex, then `CLAUDE_PLUGIN_DATA`. */
export function pluginData(options: HostOptions = {}): string | undefined {
  const { env, host } = resolveHost(options);
  const own = host === "codex" ? envValue(env, "PLUGIN_DATA") : undefined;
  return own ?? envValue(env, "CLAUDE_PLUGIN_DATA");
}

function requireNonEmpty(fn: string, values: Record<string, string>): void {
  for (const [key, value] of Object.entries(values)) {
    if (value === "") {
      throw new TypeError(`${fn}: ${key} must be non-empty`);
    }
  }
}

const INVOCATION: Readonly<Record<HostName, (namespace: string, name: string) => string>> = {
  claude: (namespace, name) => `/${namespace}:${name}`,
  codex: (namespace, name) => `$${namespace}:${name}`,
  cursor: (namespace, name) => `/${namespace}:${name}`,
  hermes: (namespace, name) => `/${namespace}:${name}`,
  // Generated OpenCode commands are named `<namespace>--<name>`.
  opencode: (namespace, name) => `/${namespace}--${name}`,
};

/** How a user invokes skill/command `name` of plugin `namespace` on this host. */
export function invocation(namespace: string, name: string, options: HostOptions = {}): string {
  requireNonEmpty("invocation", { namespace, name });
  return INVOCATION[resolveHost(options).host](namespace, name);
}

const INSTALL: Readonly<Record<HostName, ((spec: string) => string) | null>> = {
  claude: (spec) => `/plugin install ${spec}`,
  codex: (spec) => `codex plugin add ${spec}`,
  cursor: null,
  hermes: null,
  opencode: null,
};

/** Exact host-native install command for `spec`, or `null` where none exists yet. */
export function pluginInstallCommand(spec: string, options: HostOptions = {}): string | null {
  requireNonEmpty("pluginInstallCommand", { spec });
  return INSTALL[resolveHost(options).host]?.(spec) ?? null;
}
