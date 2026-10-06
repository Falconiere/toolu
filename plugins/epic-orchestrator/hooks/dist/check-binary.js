// @bun
// packages/toolu-core/src/state/state-io.ts
function toJqJson(value, pretty) {
  const json = pretty ? JSON.stringify(value, null, 2) : JSON.stringify(value);
  return json.replaceAll("\x7F", "\\u007f");
}

// packages/toolu-core/src/startup/context.ts
var MAX_CONTEXT_CHARS = 1e4;
function bounded(text, max) {
  if (text.length <= max)
    return text;
  const code = text.charCodeAt(max - 1);
  const end = code >= 55296 && code <= 56319 ? max - 1 : max;
  return text.slice(0, end);
}
function sessionContext(event, text) {
  if (text === "")
    return;
  return {
    hookSpecificOutput: {
      hookEventName: event,
      additionalContext: bounded(text, MAX_CONTEXT_CHARS)
    }
  };
}
function renderHookOutput(value, pretty) {
  return `${toJqJson(value, pretty)}
`;
}
// packages/toolu-core/src/host/host-name.ts
var HOST_NAMES = ["claude", "codex", "cursor", "opencode", "hermes"];
function envValue(env, key) {
  const value = env[key];
  return value === undefined || value === "" ? undefined : value;
}
function isHostName(value) {
  return HOST_NAMES.some((host) => host === value);
}
function childEnv(env) {
  const out = {};
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined)
      out[key] = value;
  }
  return out;
}

// packages/toolu-core/src/host/host-events.ts
var HOST_EVENTS = [
  "session/start",
  "session/unload",
  "prompt",
  "pre_compact",
  "permission/evaluate",
  "tool/pre",
  "shell/pre",
  "tool/post"
];
var PASCAL = {
  "session/start": "SessionStart",
  "session/unload": "SessionEnd",
  prompt: "UserPromptSubmit",
  pre_compact: "PreCompact",
  "permission/evaluate": "PermissionRequest",
  "tool/pre": "PreToolUse",
  "shell/pre": "PreToolUse",
  "tool/post": "PostToolUse"
};
var TABLES = {
  claude: PASCAL,
  codex: PASCAL,
  cursor: {
    "session/start": "sessionStart",
    "session/unload": "sessionEnd",
    prompt: "beforeSubmitPrompt",
    pre_compact: "preCompact",
    "permission/evaluate": null,
    "tool/pre": "preToolUse",
    "shell/pre": "beforeShellExecution",
    "tool/post": "postToolUse"
  },
  hermes: {
    "session/start": "on_session_start",
    "session/unload": "on_session_end",
    prompt: "pre_llm_call",
    pre_compact: null,
    "permission/evaluate": null,
    "tool/pre": "pre_tool_call",
    "shell/pre": "pre_tool_call",
    "tool/post": "post_tool_call"
  },
  opencode: {
    "session/start": "session.created",
    "session/unload": "session.deleted",
    prompt: "chat.message",
    pre_compact: "experimental.session.compacting",
    "permission/evaluate": null,
    "tool/pre": "tool.execute.before",
    "shell/pre": "tool.execute.before",
    "tool/post": "tool.execute.after"
  }
};
var ALIASES = {
  cursor: {
    beforeMCPExecution: "tool/pre",
    afterFileEdit: "tool/post",
    afterShellExecution: "tool/post",
    afterMCPExecution: "tool/post"
  }
};
function canonicalEvent(host, native) {
  const found = HOST_EVENTS.find((event) => TABLES[host][event] === native);
  return found ?? ALIASES[host]?.[native] ?? null;
}
function hostsForNativeEvent(native) {
  return HOST_NAMES.filter((host) => canonicalEvent(host, native) !== null);
}

// packages/toolu-core/src/host/host-detect.ts
function stderrLine(line) {
  process.stderr.write(`${line}
`);
}
function detectHost(options = {}) {
  const env = options.env ?? process.env;
  const override = envValue(env, "TOOLU_HOST_OVERRIDE");
  if (override !== undefined) {
    if (isHostName(override)) {
      return override;
    }
    (options.warn ?? stderrLine)(`toolu-host: invalid TOOLU_HOST_OVERRIDE '${override}' (using environment detection)`);
  }
  if (options.inProcess === "opencode") {
    return "opencode";
  }
  const owners = options.hookEventName ? hostsForNativeEvent(options.hookEventName) : [];
  const [owner] = owners;
  if (owners.length === 1 && owner !== undefined) {
    return owner;
  }
  if (envValue(env, "CURSOR_VERSION") ?? envValue(env, "CURSOR_PROJECT_DIR")) {
    return "cursor";
  }
  return envValue(env, "PLUGIN_ROOT") ? "codex" : "claude";
}

// packages/toolu-core/src/host/host-roots.ts
import { homedir } from "os";
import { join } from "path";
function resolveHost(options) {
  const env = options.env ?? process.env;
  return { ...options, env, host: options.host ?? detectHost({ env }) };
}
function home(env) {
  return envValue(env, "HOME") ?? homedir();
}
var NATIVE_CONFIG_ROOT = {
  claude: (env) => envValue(env, "CLAUDE_CONFIG_DIR") ?? join(home(env), ".claude"),
  codex: (env) => envValue(env, "CODEX_HOME") ?? join(home(env), ".codex"),
  cursor: (env) => join(home(env), ".cursor"),
  hermes: (env) => envValue(env, "HERMES_HOME") ?? join(home(env), ".hermes"),
  opencode: (env) => envValue(env, "TOOLU_OPENCODE_HOME") ?? join(envValue(env, "XDG_CONFIG_HOME") ?? join(home(env), ".config"), "opencode")
};
function configRoot(options = {}) {
  const { env, host } = resolveHost(options);
  return envValue(env, "TOOLU_CONFIG_DIR") ?? NATIVE_CONFIG_ROOT[host](env);
}
// packages/toolu-core/src/startup/native-toolu.ts
import { spawnSync } from "child_process";
import { createHash } from "crypto";
import { accessSync, closeSync, constants, mkdirSync, openSync, statSync } from "fs";
import { isAbsolute, join as join2, resolve } from "path";
var INSTALLER = "curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash";
var HOMEBREW = "brew install falconiere/tap/toolu";
var INSTALL_PATHS = [
  "/opt/homebrew/bin/toolu",
  "/usr/local/bin/toolu",
  "/home/linuxbrew/.linuxbrew/bin/toolu"
];
var PROBE_TIMEOUT_MS = 1000;
function shellQuote(path) {
  return `'${path.replaceAll("'", "'\\''")}'`;
}
function executable(path) {
  try {
    if (!statSync(path).isFile())
      return false;
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
function native(path, env) {
  if (!executable(path))
    return false;
  const result = spawnSync(path, ["--hook-protocol"], {
    env: childEnv(env),
    timeout: PROBE_TIMEOUT_MS,
    encoding: "utf8"
  });
  return result.error === undefined && result.status === 0 && /^[1-9][0-9]*$/u.test(result.stdout.trim());
}
function shellToolu(env) {
  const found = spawnSync("/bin/sh", ["-c", "command -v toolu"], {
    env: childEnv(env),
    timeout: PROBE_TIMEOUT_MS,
    encoding: "utf8"
  });
  if (found.error !== undefined || found.status !== 0)
    return;
  const text = found.stdout.trim();
  if (text === "" || text.includes(`
`))
    return;
  return isAbsolute(text) ? text : resolve(text);
}
function knownToolu(env) {
  const home = envValue(env, "HOME");
  const override = envValue(env, "TOOLU_BIN");
  const candidates = [
    override,
    ...INSTALL_PATHS,
    home === undefined ? undefined : join2(home, ".local/bin/toolu")
  ];
  return candidates.filter((path) => path !== undefined).map((path) => resolve(path)).find((path) => native(path, env));
}
function firstNotice(sessionId, env) {
  if (sessionId === undefined || sessionId === "")
    return true;
  const dir = join2(configRoot({ env }), "toolu", "native-notices");
  const name = createHash("sha256").update(sessionId).digest("hex");
  try {
    mkdirSync(dir, { recursive: true, mode: 448 });
    closeSync(openSync(join2(dir, name), "wx", 384));
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "EEXIST")
      return false;
    return true;
  }
}
function nativeTooluAdvice(options = {}) {
  const env = options.env ?? process.env;
  const shell = shellToolu(env);
  if (shell !== undefined && native(shell, env))
    return;
  const known = knownToolu(env);
  const line = known === undefined ? `toolu: native binary not found in the agent command shell. Install it with: ${INSTALLER} or ${HOMEBREW}. Restart the session.` : `toolu: native binary for this session: ${shellQuote(known)}. Use that absolute path for toolu commands.`;
  return firstNotice(options.sessionId, env) ? line : undefined;
}
function idFromInput(input, env) {
  try {
    const raw = JSON.parse(input);
    if (raw !== null && typeof raw === "object" && "session_id" in raw) {
      const id = raw.session_id;
      if (typeof id === "string" && id !== "")
        return id;
    }
  } catch {}
  return envValue(env, "TOOLU_SESSION_ID");
}
async function runNativeTooluCheck(env = process.env) {
  const input = await Bun.stdin.text();
  const line = nativeTooluAdvice({ env, sessionId: idFromInput(input, env) });
  if (line !== undefined) {
    process.stdout.write(renderHookOutput(sessionContext("SessionStart", line), false));
  }
}
// plugins/epic-orchestrator/hooks/src/check-binary.ts
await runNativeTooluCheck();
