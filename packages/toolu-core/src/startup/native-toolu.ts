/** SessionStart advice for the native CLI in an agent's non-login command shell. */
import { spawnSync } from "node:child_process";
import { accessSync, closeSync, constants, mkdirSync, openSync, statSync } from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { childEnv, envValue, type HostEnv } from "../host/host-name.ts";
import { configRoot } from "../host/host-roots.ts";
import { renderHookOutput, sessionContext } from "./context.ts";

const INSTALLER = "curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash";
const HOMEBREW = "brew install falconiere/tap/toolu";
const INSTALL_PATHS = [
  "/opt/homebrew/bin/toolu",
  "/usr/local/bin/toolu",
  "/home/linuxbrew/.linuxbrew/bin/toolu",
];
const PROBE_TIMEOUT_MS = 1_000;

export type NativeTooluOptions = { env?: HostEnv; sessionId?: string | undefined };

/** A path the agent can copy into a POSIX command, including spaces or quotes. */
function shellQuote(path: string): string {
  return `'${path.replaceAll("'", "'\\''")}'`;
}

function executable(path: string): boolean {
  try {
    if (!statSync(path).isFile()) return false;
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/** The same native marker the generated hook launcher checks (#412). */
function native(path: string, env: HostEnv): boolean {
  if (!executable(path)) return false;
  const result = spawnSync(path, ["--hook-protocol"], {
    env: childEnv(env),
    timeout: PROBE_TIMEOUT_MS,
    encoding: "utf8",
  });
  return (
    result.error === undefined && result.status === 0 && /^[1-9][0-9]*$/u.test(result.stdout.trim())
  );
}

/** What a non-login shell will actually run for plain `toolu`. */
function shellToolu(env: HostEnv): string | undefined {
  const found = spawnSync("/bin/sh", ["-c", "command -v toolu"], {
    env: childEnv(env),
    timeout: PROBE_TIMEOUT_MS,
    encoding: "utf8",
  });
  if (found.error !== undefined || found.status !== 0) return undefined;
  const text = found.stdout.trim();
  if (text === "" || text.includes("\n")) return undefined;
  return isAbsolute(text) ? text : resolve(text);
}

function knownToolu(env: HostEnv): string | undefined {
  const home = envValue(env, "HOME");
  const override = envValue(env, "TOOLU_BIN");
  const candidates = [
    override,
    ...INSTALL_PATHS,
    home === undefined ? undefined : join(home, ".local/bin/toolu"),
  ];
  return candidates
    .filter((path): path is string => path !== undefined)
    .map((path) => resolve(path))
    .find((path) => native(path, env));
}

/** FNV-1a 64, matching `toolu_runtime::startup` notice filenames. */
function noticeName(sessionId: string): string {
  let hash = 0xcbf29ce484222325n;
  const mask = 0xffffffffffffffffn;
  for (const byte of new TextEncoder().encode(sessionId)) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & mask;
  }
  return hash.toString(16).padStart(16, "0");
}

/** True only for the first notice under one host config root and session ID. */
function firstNotice(sessionId: string | undefined, env: HostEnv): boolean {
  if (sessionId === undefined || sessionId === "") return true;
  const dir = join(configRoot({ env }), "toolu", "native-notices");
  const name = noticeName(sessionId);
  try {
    mkdirSync(dir, { recursive: true, mode: 0o700 });
    closeSync(openSync(join(dir, name), "wx", 0o600));
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "EEXIST") return false;
    // Advice must still reach the agent when the marker cannot be written.
    return true;
  }
}

/** No line when plain `toolu` is native; otherwise one actionable session line. */
export function nativeTooluAdvice(options: NativeTooluOptions = {}): string | undefined {
  const env = options.env ?? process.env;
  const shell = shellToolu(env);
  if (shell !== undefined && native(shell, env)) return undefined;
  const known = knownToolu(env);
  const line =
    known === undefined
      ? `toolu: native binary not found in the agent command shell. Install it with: ${INSTALLER} or ${HOMEBREW}. Restart the session.`
      : `toolu: native binary for this session: ${shellQuote(known)}. Use that absolute path for toolu commands.`;
  return firstNotice(options.sessionId, env) ? line : undefined;
}

function idFromInput(input: string, env: HostEnv): string | undefined {
  try {
    const raw: unknown = JSON.parse(input);
    if (raw !== null && typeof raw === "object" && "session_id" in raw) {
      const id = raw.session_id;
      if (typeof id === "string" && id !== "") return id;
    }
  } catch {
    // A malformed payload still gets the install hint.
  }
  return envValue(env, "TOOLU_SESSION_ID");
}

/** The tiny entry each agent-facing plugin bundles as `check-binary.js`. */
export async function runNativeTooluCheck(env: HostEnv = process.env): Promise<void> {
  const input = await Bun.stdin.text();
  const line = nativeTooluAdvice({ env, sessionId: idFromInput(input, env) });
  if (line !== undefined) {
    process.stdout.write(renderHookOutput(sessionContext("SessionStart", line), false));
  }
}
