// @bun
// plugins/context7/hooks/src/session-start.ts
import { lstatSync as lstatSync2 } from "fs";
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

// packages/toolu-core/src/startup/report.ts
import { appendFileSync } from "fs";
var STARTUP_REPORT_ENV = "TOOLU_STARTUP_REPORT";
function reportStartup(record, env = process.env) {
  const path = envValue(env, STARTUP_REPORT_ENV);
  if (path === undefined)
    return;
  try {
    appendFileSync(path, `${JSON.stringify(record)}
`);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    process.stderr.write(`toolu-startup: cannot write startup report ${path}: ${reason}
`);
    process.exitCode = 1;
  }
}

// packages/toolu-core/src/startup/publish.ts
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
function helperRecord(options, result) {
  const record = { kind: "helper", plugin: options.plugin, source: options.source };
  if (result.status === "source-missing")
    return { ...record, status: result.status };
  return { ...record, path: result.path, status: result.status };
}
function publishWrapper(options) {
  const result = publish(options);
  reportStartup(helperRecord(options, result), options.env ?? process.env);
  return result;
}
function publish(options) {
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
// plugins/context7/hooks/src/context7/opencode.ts
var OPENCODE_SKILL = "context7-context7";
function onOpencode() {
  return process.env.TOOLU_HOST_OVERRIDE === "opencode";
}
var quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
function command(path, symlink) {
  return symlink ? `${quote(process.execPath)} --no-env-file ${quote(path)}` : quote(path);
}
function instruction(run) {
  return `context7 (library docs) \u2014 for ANY third-party library/framework question (API usage, current docs, code examples, version behavior) you MUST run ${run} FIRST (\`search <library>\` to resolve the ID, then \`docs <id> <query>\`) BEFORE answering from memory or searching the web. Web search is a FALLBACK ONLY when context7 lacks coverage or the command exits non-zero (22 is an HTTP error such as a 429 rate limit). Syntax: skill({ name: "${OPENCODE_SKILL}" }). CONTEXT7_API_KEY is optional and read from the environment only, never from .env.`;
}
async function compacting() {
  let input;
  try {
    input = JSON.parse(await Bun.stdin.text());
  } catch {
    return false;
  }
  return input !== null && typeof input === "object" && "source" in input && input.source === "compact";
}

// plugins/context7/hooks/src/session-start.ts
var options = {
  plugin: "context7",
  source: resolve(import.meta.dir, "../dist/search.js"),
  dir: "context7",
  name: "search.sh"
};
if (onOpencode()) {
  const quiet = await compacting();
  const result = publishWrapper(options);
  if (!quiet && (result.status === "published" || result.status === "kept-user-file")) {
    const symlink = lstatSync2(result.path).isSymbolicLink();
    process.stdout.write(renderHookOutput(sessionContext("SessionStart", instruction(command(result.path, symlink))), false));
  }
} else {
  publishBunCli({ ...options, tool: "context7 search CLI" });
}
