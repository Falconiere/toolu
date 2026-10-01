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
// packages/toolu-core/src/startup/dependencies.ts
import { spawnSync } from "child_process";

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
    "permission/evaluate": "permission.evaluate",
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
function resolveHost(options) {
  const env = options.env ?? process.env;
  return { ...options, env, host: options.host ?? detectHost({ env }) };
}
function requireNonEmpty(fn, values) {
  for (const [key, value] of Object.entries(values)) {
    if (value === "") {
      throw new TypeError(`${fn}: ${key} must be non-empty`);
    }
  }
}
var INSTALL = {
  claude: (spec) => `/plugin install ${spec}`,
  codex: (spec) => `codex plugin add ${spec}`,
  cursor: null,
  hermes: null,
  opencode: null
};
function pluginInstallCommand(spec, options = {}) {
  requireNonEmpty("pluginInstallCommand", { spec });
  return INSTALL[resolveHost(options).host]?.(spec) ?? null;
}

// packages/toolu-core/src/startup/dependencies.ts
var CORE_PLUGIN = "toolu@toolu";
function codexListing(env) {
  const res = spawnSync("codex", ["plugin", "list", "--json"], {
    env: childEnv(env),
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"]
  });
  if (res.error !== undefined || res.status !== 0)
    return;
  try {
    const value = JSON.parse(res.stdout);
    return { ok: true, value };
  } catch {
    return { ok: false };
  }
}
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function flagOn(entry, key) {
  return !Object.hasOwn(entry, key) || entry[key] === true;
}
function entries(listing) {
  if (!isRecord(listing))
    return [];
  const installed = listing["installed"];
  if (Array.isArray(installed))
    return installed;
  return isRecord(installed) ? Object.values(installed) : [];
}
function isInstalled(listing, id) {
  for (const entry of entries(listing)) {
    if (entry === null)
      continue;
    if (!isRecord(entry))
      return false;
    if (entry["pluginId"] === id && flagOn(entry, "installed") && flagOn(entry, "enabled")) {
      return true;
    }
  }
  return false;
}
function codexMissingPlugins(required, env = process.env) {
  if (detectHost({ env }) !== "codex" || envValue(env, "PLUGIN_ROOT") === undefined) {
    return;
  }
  const listing = codexListing(env);
  if (listing === undefined)
    return;
  return required.filter((id) => !listing.ok || !isInstalled(listing.value, id));
}
function installCommand(spec) {
  return pluginInstallCommand(spec, { host: "codex" }) ?? spec;
}
function requiresCoreWarning() {
  return `WARN: this plugin requires the toolu core. Install it first with: ${installCommand(CORE_PLUGIN)}`;
}
function requiresPluginsWarning(missing) {
  const parts = missing.map((id) => ` ${id} (install with: ${installCommand(id)})`);
  return `WARN: this plugin requires${parts.join("")}`;
}
function codexDependencyNotice(required, style, env = process.env) {
  const missing = codexMissingPlugins(required, env);
  if (missing === undefined || missing.length === 0)
    return;
  const text = style === "core" ? requiresCoreWarning() : requiresPluginsWarning(missing);
  return renderHookOutput(sessionContext("SessionStart", text), true);
}
// plugins/epic-orchestrator/hooks/src/check-deps.ts
var REQUIRED = [
  CORE_PLUGIN,
  "delivery-flow@toolu",
  "toolu-review@toolu",
  "pr-babysit@toolu",
  "brainstorm@toolu"
];
var notice = codexDependencyNotice(REQUIRED, "each");
if (notice !== undefined)
  process.stdout.write(notice);
