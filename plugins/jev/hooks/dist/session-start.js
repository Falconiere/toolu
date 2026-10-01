// @bun
// plugins/jev/hooks/src/session-start.ts
import { accessSync, constants } from "fs";
import { resolve } from "path";

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
// packages/toolu-core/src/startup/publish.ts
import { randomUUID } from "crypto";
import {
  lstatSync,
  mkdirSync,
  readlinkSync,
  renameSync,
  rmSync,
  statSync,
  symlinkSync
} from "fs";
import { join as join2 } from "path";
function stderrLine2(line) {
  process.stderr.write(`${line}
`);
}
function isFile(path) {
  return statSync(path, { throwIfNoEntry: false })?.isFile() ?? false;
}
function relink(source, dst) {
  const tmp = `${dst}.${randomUUID()}.tmp`;
  try {
    symlinkSync(source, tmp);
    renameSync(tmp, dst);
    return true;
  } catch {
    rmSync(tmp, { force: true });
    return false;
  }
}
function publishWrapper(options) {
  if (!isFile(options.source)) {
    return { status: "source-missing" };
  }
  const env = options.env ?? process.env;
  const dir = join2(configRoot({ env }), options.dir);
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    (options.warn ?? stderrLine2)(`${options.plugin}: cannot create ${dir} \u2014 ${options.what ?? "wrapper"} not published`);
    return { status: "unwritable", path: dir };
  }
  const path = join2(dir, options.name);
  const existing = lstatSync(path, { throwIfNoEntry: false });
  if (existing !== undefined && !existing.isSymbolicLink()) {
    return { status: "kept-user-file", path };
  }
  if (existing !== undefined && readlinkSync(path) === options.source) {
    return { status: "published", path };
  }
  return { status: relink(options.source, path) ? "published" : "link-failed", path };
}
function bunOnPath(env = process.env) {
  return Bun.which("bun", { PATH: envValue(env, "PATH") ?? "" }) !== null;
}
function bunAdvisory(plugin, tool) {
  return `${plugin}: bun not found on PATH \u2014 the ${tool} needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)`;
}
function publishBunCli(options) {
  const result = publishWrapper(options);
  const ran = result.status !== "source-missing" && result.status !== "unwritable";
  if (ran && !bunOnPath(options.env ?? process.env)) {
    (options.warn ?? stderrLine2)(bunAdvisory(options.plugin, options.tool));
  }
  return result;
}
// plugins/jev/hooks/src/session-start.ts
var PLUGIN = resolve(import.meta.dir, "../..");
function executable(path) {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
function missingPrereqs(wrapper) {
  const missing = [];
  if (!bunOnPath())
    missing.push("bun");
  if (!process.env.TYPESAFE_API_KEY)
    missing.push("TYPESAFE_API_KEY");
  if (!executable(wrapper))
    missing.push("executable-wrapper");
  return missing.map((item) => ` ${item}`).join("");
}
function mandate(wrapper) {
  const missing = missingPrereqs(wrapper);
  if (missing !== "") {
    return `Jev unavailable (missing:${missing}). Set TYPESAFE_API_KEY in the agent's launch environment and install Bun 1.4.x. Jev is mandatory on every task once available; until then, state the limitation once per task and use an explicit reasoning/evidence fallback; never invent a Jev result. Do not read credentials from .env.`;
  }
  return `Jev is mandatory on every task containing semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call "${wrapper}" before the decision it informs. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: ${PLUGIN}/skills/jev/SKILL.md. Keep exact rules, tests, and code verification deterministic. On service failure, state the limitation and use an explicit evidence fallback. Jev never replaces tests or authorization.`;
}
var result = publishBunCli({
  plugin: "jev",
  source: resolve(PLUGIN, "hooks/dist/jev.js"),
  dir: "jev",
  name: "jev.sh",
  tool: "jev CLI"
});
if (result.status === "link-failed") {
  process.stderr.write(`jev: cannot publish ${result.path}
`);
} else if (result.status === "published" || result.status === "kept-user-file") {
  process.stdout.write(renderHookOutput(sessionContext("SessionStart", mandate(result.path)), false));
}
