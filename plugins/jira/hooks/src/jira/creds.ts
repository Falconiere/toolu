/**
 * Credential discovery from an installed ankitpokhrel jira-cli: server and
 * login from its flat YAML config, the API token in jira-cli's own order
 * (config api_token → ~/.netrc → OS keyring). It only fills JIRA_* variables
 * that are unset; explicit env always wins, and a missing CLI is a no-op.
 */
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

export type Env = Record<string, string | undefined>;

/** File contents, or undefined when it cannot be read (bash `[[ -r ]]`). */
function readable(path: string): string | undefined {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
}

/** A top-level `key: value` scalar: the first line starting `key:`, trimmed. */
function yamlGet(content: string, key: string): string {
  const line = content.split("\n").find((candidate) => candidate.startsWith(`${key}:`));
  return line === undefined ? "" : line.replace(/^[^:]*:\s*/, "").replace(/\s+$/, "");
}

/** The password of `host`'s `machine` entry, with tokens flattened across lines. */
function netrcToken(env: Env, host: string): string {
  const content = readable(env["NETRC"] || join(env["HOME"] ?? "", ".netrc"));
  if (content === undefined) return "";
  const tokens = content.split(/\s+/).filter((token) => token !== "");
  let inHost = false;
  for (let at = 0; at < tokens.length; at += 1) {
    if (tokens[at] === "machine") inHost = tokens[at + 1] === host;
    if (inHost && tokens[at] === "password") return tokens[at + 1] ?? "";
  }
  return "";
}

/** A keyring helper's stdout with trailing newlines trimmed, as `$(…)` kept it; empty on any miss. */
function helperOutput(env: Env, command: string, args: readonly string[]): string {
  const run = spawnSync(command, args, {
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  return run.error === undefined ? run.stdout.replace(/\n+$/, "") : "";
}

/** Best-effort token from macOS `security`, then Linux `secret-tool`. */
function keyringToken(env: Env, login: string): string {
  const service = env["JIRA_KEYRING_SERVICE"] || "jira-cli";
  const token = helperOutput(env, "security", [
    "find-generic-password",
    "-s",
    service,
    "-a",
    login,
    "-w",
  ]);
  if (token !== "") return token;
  return helperOutput(env, "secret-tool", ["lookup", "service", service, "username", login]);
}

function configPath(env: Env): string {
  const config = env["XDG_CONFIG_HOME"] || join(env["HOME"] ?? "", ".config");
  return env["JIRA_CLI_CONFIG"] || join(config, ".jira", ".config.yml");
}

function tokenFor(env: Env, content: string, server: string, login: string): string {
  let token = yamlGet(content, "api_token");
  if (token === "" && server !== "") {
    const host = server.replace(/^.*?:\/\//, "").replace(/\/.*$/, "");
    token = netrcToken(env, host);
  }
  if (token === "" && login !== "") token = keyringToken(env, login);
  return token;
}

/** `env` with unset JIRA_* filled from the jira-cli config; the input is not modified. */
export function withCliCreds(env: Env): Env {
  const content = readable(configPath(env));
  if (content === undefined) return env;
  const filled: Env = { ...env };
  const server = yamlGet(content, "server");
  const login = yamlGet(content, "login");
  const installation = yamlGet(content, "installation");
  if (!filled["JIRA_BASE_URL"] && server !== "") filled["JIRA_BASE_URL"] = server;
  if (!filled["JIRA_EMAIL"] && login !== "") filled["JIRA_EMAIL"] = login;
  if (!filled["JIRA_API_VERSION"] && installation !== "") {
    filled["JIRA_API_VERSION"] = /^[Cc]loud$/.test(installation) ? "3" : "2";
  }
  if (!filled["JIRA_PAT"] && !filled["JIRA_API_TOKEN"]) {
    const token = tokenFor(filled, content, server, login);
    if (token !== "") filled["JIRA_API_TOKEN"] = token;
  }
  return filled;
}
