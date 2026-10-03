#!/usr/bin/env bun
// @bun

// packages/toolu-core/src/ledger/ledger-commands.ts
import { accessSync, constants as constants3, mkdtempSync, rmSync as rmSync4, writeFileSync as writeFileSync3 } from "fs";
import { tmpdir } from "os";
import { isAbsolute, join as join7, resolve as resolve3 } from "path";

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

// packages/toolu-core/src/host/host-roots.ts
import { spawnSync } from "child_process";
import { homedir } from "os";
import { join } from "path";

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
function gitToplevel(env, cwd) {
  const res = spawnSync("git", ["rev-parse", "--show-toplevel"], {
    cwd: cwd ?? process.cwd(),
    env: childEnv(env),
    encoding: "utf8"
  });
  if (res.error !== undefined || res.status !== 0) {
    return;
  }
  const top = res.stdout.trim();
  return top === "" ? undefined : top;
}
var PROJECT_DIR_VAR = {
  claude: "CLAUDE_PROJECT_DIR",
  cursor: "CURSOR_PROJECT_DIR"
};
function projectRoot(options = {}) {
  const { env, host } = resolveHost(options);
  const hostVar = PROJECT_DIR_VAR[host];
  return envValue(env, "TOOLU_PROJECT_DIR") ?? (hostVar === undefined ? undefined : envValue(env, hostVar)) ?? gitToplevel(env, options.cwd);
}
function projectDirname(options = {}) {
  const { env, host } = resolveHost(options);
  return envValue(env, "TOOLU_PROJECT_CONFIG_DIRNAME") ?? `.${host}`;
}
function projectConfigPath(options = {}) {
  const o = resolveHost(options);
  const root = projectRoot(o);
  return root === undefined ? undefined : join(root, projectDirname(o), "toolu.config.json");
}
function projectStateRoot(options = {}) {
  const o = resolveHost(options);
  const root = o.root ?? projectRoot(o);
  return root === undefined ? undefined : join(root, projectDirname(o), "tmp");
}
function projectStateDir(name, options = {}) {
  if (name === "") {
    throw new TypeError("projectStateDir: name must be non-empty");
  }
  const base = projectStateRoot(resolveHost(options));
  return base === undefined ? undefined : join(base, name);
}

// packages/toolu-core/src/state/diff-sha.ts
function diffSha(repoRoot, baseRef, options = {}) {
  if (baseRef.startsWith("-"))
    return;
  const env = childEnv(options.env ?? process.env);
  const diff = Bun.spawnSync(["git", "-C", repoRoot, "diff", "--no-color", `${baseRef}...HEAD`], {
    env,
    stdout: "pipe",
    stderr: "ignore"
  });
  if (!diff.success)
    return;
  const hash = Bun.spawnSync(["git", "-C", repoRoot, "hash-object", "--stdin"], {
    env,
    stdin: diff.stdout,
    stdout: "pipe",
    stderr: "ignore"
  });
  if (!hash.success)
    return;
  const sha = hash.stdout.toString("utf8").trim();
  return sha === "" ? undefined : sha;
}

// packages/toolu-core/src/state/state-git.ts
import { spawnSync as spawnSync3 } from "child_process";

// packages/toolu-core/src/detect/detect-branch.ts
import { spawnSync as spawnSync2 } from "child_process";
function branchSlug(branch) {
  const slug = branch.replaceAll("/", "_").replace(/[^A-Za-z0-9_-]/g, "");
  return slug === "" ? "_default" : slug;
}
function baseBranch(root, env = process.env, cwd) {
  const top = root === undefined || root === "" ? gitToplevel(env, cwd) : root;
  if (top === undefined)
    return "main";
  const res = spawnSync2("git", ["-C", top, "symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"], {
    env: childEnv(env),
    encoding: "utf8"
  });
  const ref = res.error === undefined && res.status === 0 ? res.stdout.trim() : "";
  return ref === "" ? "main" : ref.replace(/^refs\/remotes\/origin\//, "");
}
// packages/toolu-core/src/state/state-git.ts
function hasGit(env) {
  const res = spawnSync3("git", ["--version"], { env: childEnv(env), encoding: "utf8" });
  return res.error === undefined && res.status === 0;
}
function currentBranch(root, env) {
  const res = spawnSync3("git", ["-C", root, "rev-parse", "--abbrev-ref", "HEAD"], {
    env: childEnv(env),
    encoding: "utf8"
  });
  return res.error === undefined ? res.stdout.replace(/\n+$/, "") : "";
}

// packages/toolu-core/src/state/state-io.ts
var stderrWarn = (message) => {
  console.error(message);
};
function toJqJson(value, pretty) {
  const json = pretty ? JSON.stringify(value, null, 2) : JSON.stringify(value);
  return json.replaceAll("\x7F", "\\u007f");
}
function isoSeconds(date) {
  return `${date.toISOString().slice(0, 19)}Z`;
}
function compareJqStrings(a, b) {
  return Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));
}

// packages/toolu-core/src/ledger/ledger-io.ts
import { spawnSync as spawnSync4 } from "child_process";
import { mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "fs";
import { dirname, join as join2 } from "path";

// packages/toolu-core/src/ledger/ledger-jq.ts
class JqError extends Error {
  name = "JqError";
}
function jqType(value) {
  if (value === null)
    return "null";
  if (Array.isArray(value))
    return "array";
  return typeof value === "object" ? "object" : typeof value;
}
function isObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function get(value, key) {
  if (value === null)
    return null;
  if (isObject(value) && typeof key === "string") {
    return Object.hasOwn(value, key) ? value[key] ?? null : null;
  }
  throw new JqError(`Cannot index ${jqType(value)} with ${jqType(key)}`);
}
function each(value) {
  if (Array.isArray(value))
    return value;
  if (isObject(value))
    return Object.values(value);
  throw new JqError(`Cannot iterate over ${jqType(value)}`);
}
function eachOptional(value) {
  return Array.isArray(value) || isObject(value) ? each(value) : [];
}
function alt(value, fallback) {
  return value === undefined || value === null || value === false ? fallback : value;
}
function truthy(value) {
  return value !== undefined && value !== null && value !== false;
}
function jqEquals(a, b) {
  if (a === b)
    return true;
  if (Array.isArray(a) || Array.isArray(b)) {
    return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((item, i) => jqEquals(item, b[i] ?? null));
  }
  if (isObject(a) && isObject(b)) {
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every((key) => Object.hasOwn(b, key) && jqEquals(a[key] ?? null, b[key] ?? null));
  }
  return false;
}
function length(value) {
  if (value === null)
    return 0;
  if (typeof value === "boolean")
    throw new JqError("boolean has no length");
  if (typeof value === "number")
    return Math.abs(value);
  if (typeof value === "string")
    return value.match(/[\s\S]/gu)?.length ?? 0;
  return Array.isArray(value) ? value.length : Object.keys(value).length;
}
function jqIndex(container, needle) {
  if (container === null)
    return null;
  if (Array.isArray(container)) {
    const at = container.findIndex((item) => jqEquals(item, needle));
    return at === -1 ? null : at;
  }
  if (typeof container === "string") {
    const at = container.indexOf(needle);
    return at === -1 ? null : at;
  }
  const found = get(container, needle);
  if (found === null)
    return null;
  if (Array.isArray(found))
    return found[0] ?? null;
  throw new JqError(`Cannot index ${jqType(found)} with number`);
}
function holds(container, needle) {
  return truthy(jqIndex(container, needle));
}
function toStr(value) {
  return typeof value === "string" ? value : toJqJson(value, false);
}
function raw(value) {
  return typeof value === "string" ? value : toJqJson(value, true);
}
function concat(...parts) {
  let out = "";
  for (const part of parts) {
    if (part === null)
      continue;
    if (typeof part !== "string")
      throw new JqError(`string and ${jqType(part)} cannot be added`);
    out += part;
  }
  return out;
}
function isJson(value) {
  if (value === null || ["string", "number", "boolean"].includes(typeof value))
    return true;
  if (Array.isArray(value))
    return value.every(isJson);
  return typeof value === "object" && Object.values(value).every(isJson);
}
function parseJson(text) {
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    return;
  }
  return isJson(value) ? value : undefined;
}

// packages/toolu-core/src/ledger/ledger-io.ts
function readLedger(file) {
  let text;
  try {
    if (statSync(file).size === 0)
      return;
    text = readFileSync(file, "utf8");
  } catch {
    return;
  }
  const value = parseJson(text);
  if (value === undefined || value === null || value === false)
    return;
  return { value, text: text.replace(/\n+$/, "") };
}
function writeLedger(file, ledger) {
  const dir = dirname(file);
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    return `plan-ledger-parse: cannot create ledger dir: ${dir}`;
  }
  const tmp = `${file}.tmp.${process.pid}`;
  const body = `${toJqJson(ledger, true)}
`;
  try {
    writeFileSync(tmp, body);
  } catch {
    rmSync(tmp, { force: true });
    return `plan-ledger-parse: failed to stage ledger to ${tmp}`;
  }
  try {
    renameSync(tmp, file);
  } catch {
    rmSync(tmp, { force: true });
    return `plan-ledger-parse: atomic mv failed for ${file}`;
  }
  return;
}
function projectRoot2(options = {}) {
  return gitToplevel(options.env ?? process.env, options.cwd);
}
function headBranch(options = {}) {
  const res = spawnSync4("git", ["rev-parse", "--abbrev-ref", "HEAD"], {
    cwd: options.cwd ?? process.cwd(),
    env: childEnv(options.env ?? process.env),
    encoding: "utf8"
  });
  return res.error === undefined && res.status === 0 ? res.stdout.replace(/\n+$/, "") : undefined;
}
function stateRootFor(root, options = {}) {
  const env = options.env ?? process.env;
  const host = options.host === undefined ? { env } : { env, host: options.host };
  return projectStateRoot({ ...host, root }) ?? join2(root, "tmp");
}
function stateDirFor(name, root, options = {}) {
  return join2(stateRootFor(root, options), name);
}
function ledgerPath(options = {}) {
  const root = projectRoot2(options);
  if (root === undefined)
    return;
  const branch = headBranch(options);
  if (branch === undefined)
    return;
  return join2(stateDirFor("plan-ledger", root, options), `${branchSlug(branch)}.json`);
}

class Output {
  onStderr;
  out = "";
  err = [];
  constructor(onStderr) {
    this.onStderr = onStderr;
  }
  stdout(text) {
    this.out += text;
  }
  stderr(line) {
    this.err.push(line);
    this.onStderr?.(line);
  }
  result(exitCode) {
    return { exitCode, stdout: this.out, stderr: this.err.map((line) => `${line}
`).join("") };
  }
}

// packages/toolu-core/src/ledger/ledger-parse.ts
import { readFileSync as readFileSync3, statSync as statSync3 } from "fs";
import { resolve } from "path";

// packages/toolu-core/src/config/config-load.ts
import { readFileSync as readFileSync2 } from "fs";

// packages/toolu-core/src/config/config-files.ts
import { statSync as statSync2 } from "fs";
import { join as join3 } from "path";
function isFile(path) {
  try {
    return statSync2(path).isFile();
  } catch {
    return false;
  }
}
function configFiles(options) {
  const env = options.env ?? process.env;
  const host = options.host ?? detectHost({ env });
  const scoped = options.cwd === undefined ? { env, host } : { env, host, cwd: options.cwd };
  const userDir = envValue(env, "TOOLU_USER_CONFIG_DIR") ?? configRoot(scoped);
  const files = {
    user: join3(userDir, "toolu.config.json"),
    project: projectConfigPath(scoped)
  };
  return { files, host };
}

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/core.js
var NEVER = Object.freeze({
  status: "aborted"
});
function $constructor(name, initializer, params) {
  function init(inst, def) {
    var _a;
    Object.defineProperty(inst, "_zod", {
      value: inst._zod ?? {},
      enumerable: false
    });
    (_a = inst._zod).traits ?? (_a.traits = new Set);
    inst._zod.traits.add(name);
    initializer(inst, def);
    for (const k in _.prototype) {
      if (!(k in inst))
        Object.defineProperty(inst, k, { value: _.prototype[k].bind(inst) });
    }
    inst._zod.constr = _;
    inst._zod.def = def;
  }
  const Parent = params?.Parent ?? Object;

  class Definition extends Parent {
  }
  Object.defineProperty(Definition, "name", { value: name });
  function _(def) {
    var _a;
    const inst = params?.Parent ? new Definition : this;
    init(inst, def);
    (_a = inst._zod).deferred ?? (_a.deferred = []);
    for (const fn of inst._zod.deferred) {
      fn();
    }
    return inst;
  }
  Object.defineProperty(_, "init", { value: init });
  Object.defineProperty(_, Symbol.hasInstance, {
    value: (inst) => {
      if (params?.Parent && inst instanceof params.Parent)
        return true;
      return inst?._zod?.traits?.has(name);
    }
  });
  Object.defineProperty(_, "name", { value: name });
  return _;
}
var $brand = Symbol("zod_brand");

class $ZodAsyncError extends Error {
  constructor() {
    super(`Encountered Promise during synchronous parse. Use .parseAsync() instead.`);
  }
}

class $ZodEncodeError extends Error {
  constructor(name) {
    super(`Encountered unidirectional transform during encode: ${name}`);
    this.name = "ZodEncodeError";
  }
}
var globalConfig = {};
function config(newConfig) {
  if (newConfig)
    Object.assign(globalConfig, newConfig);
  return globalConfig;
}
// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/util.js
function getEnumValues(entries) {
  const numericValues = Object.values(entries).filter((v) => typeof v === "number");
  const values = Object.entries(entries).filter(([k, _]) => numericValues.indexOf(+k) === -1).map(([_, v]) => v);
  return values;
}
function jsonStringifyReplacer(_, value) {
  if (typeof value === "bigint")
    return value.toString();
  return value;
}
function cached(getter) {
  const set = false;
  return {
    get value() {
      if (!set) {
        const value = getter();
        Object.defineProperty(this, "value", { value });
        return value;
      }
      throw new Error("cached value already set");
    }
  };
}
function nullish(input) {
  return input === null || input === undefined;
}
function cleanRegex(source) {
  const start = source.startsWith("^") ? 1 : 0;
  const end = source.endsWith("$") ? source.length - 1 : source.length;
  return source.slice(start, end);
}
function floatSafeRemainder(val, step) {
  const valDecCount = (val.toString().split(".")[1] || "").length;
  const stepString = step.toString();
  let stepDecCount = (stepString.split(".")[1] || "").length;
  if (stepDecCount === 0 && /\d?e-\d?/.test(stepString)) {
    const match = stepString.match(/\d?e-(\d?)/);
    if (match?.[1]) {
      stepDecCount = Number.parseInt(match[1]);
    }
  }
  const decCount = valDecCount > stepDecCount ? valDecCount : stepDecCount;
  const valInt = Number.parseInt(val.toFixed(decCount).replace(".", ""));
  const stepInt = Number.parseInt(step.toFixed(decCount).replace(".", ""));
  return valInt % stepInt / 10 ** decCount;
}
var EVALUATING = Symbol("evaluating");
function defineLazy(object, key, getter) {
  let value = undefined;
  Object.defineProperty(object, key, {
    get() {
      if (value === EVALUATING) {
        return;
      }
      if (value === undefined) {
        value = EVALUATING;
        value = getter();
      }
      return value;
    },
    set(v) {
      Object.defineProperty(object, key, {
        value: v
      });
    },
    configurable: true
  });
}
function objectClone(obj) {
  return Object.create(Object.getPrototypeOf(obj), Object.getOwnPropertyDescriptors(obj));
}
function assignProp(target, prop, value) {
  Object.defineProperty(target, prop, {
    value,
    writable: true,
    enumerable: true,
    configurable: true
  });
}
function mergeDefs(...defs) {
  const mergedDescriptors = {};
  for (const def of defs) {
    const descriptors = Object.getOwnPropertyDescriptors(def);
    Object.assign(mergedDescriptors, descriptors);
  }
  return Object.defineProperties({}, mergedDescriptors);
}
function esc(str) {
  return JSON.stringify(str);
}
var captureStackTrace = "captureStackTrace" in Error ? Error.captureStackTrace : (..._args) => {};
function isObject2(data) {
  return typeof data === "object" && data !== null && !Array.isArray(data);
}
var allowsEval = cached(() => {
  if (typeof navigator !== "undefined" && navigator?.userAgent?.includes("Cloudflare")) {
    return false;
  }
  try {
    const F = Function;
    new F("");
    return true;
  } catch (_) {
    return false;
  }
});
function isPlainObject(o) {
  if (isObject2(o) === false)
    return false;
  const ctor = o.constructor;
  if (ctor === undefined)
    return true;
  const prot = ctor.prototype;
  if (isObject2(prot) === false)
    return false;
  if (Object.prototype.hasOwnProperty.call(prot, "isPrototypeOf") === false) {
    return false;
  }
  return true;
}
function shallowClone(o) {
  if (isPlainObject(o))
    return { ...o };
  return o;
}
var propertyKeyTypes = new Set(["string", "number", "symbol"]);
var primitiveTypes = new Set(["string", "number", "bigint", "boolean", "symbol", "undefined"]);
function escapeRegex(str) {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
function clone(inst, def, params) {
  const cl = new inst._zod.constr(def ?? inst._zod.def);
  if (!def || params?.parent)
    cl._zod.parent = inst;
  return cl;
}
function normalizeParams(_params) {
  const params = _params;
  if (!params)
    return {};
  if (typeof params === "string")
    return { error: () => params };
  if (params?.message !== undefined) {
    if (params?.error !== undefined)
      throw new Error("Cannot specify both `message` and `error` params");
    params.error = params.message;
  }
  delete params.message;
  if (typeof params.error === "string")
    return { ...params, error: () => params.error };
  return params;
}
function optionalKeys(shape) {
  return Object.keys(shape).filter((k) => {
    return shape[k]._zod.optin === "optional" && shape[k]._zod.optout === "optional";
  });
}
var NUMBER_FORMAT_RANGES = {
  safeint: [Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER],
  int32: [-2147483648, 2147483647],
  uint32: [0, 4294967295],
  float32: [-340282346638528860000000000000000000000, 340282346638528860000000000000000000000],
  float64: [-Number.MAX_VALUE, Number.MAX_VALUE]
};
function pick(schema, mask) {
  const currDef = schema._zod.def;
  const def = mergeDefs(schema._zod.def, {
    get shape() {
      const newShape = {};
      for (const key in mask) {
        if (!(key in currDef.shape)) {
          throw new Error(`Unrecognized key: "${key}"`);
        }
        if (!mask[key])
          continue;
        newShape[key] = currDef.shape[key];
      }
      assignProp(this, "shape", newShape);
      return newShape;
    },
    checks: []
  });
  return clone(schema, def);
}
function omit(schema, mask) {
  const currDef = schema._zod.def;
  const def = mergeDefs(schema._zod.def, {
    get shape() {
      const newShape = { ...schema._zod.def.shape };
      for (const key in mask) {
        if (!(key in currDef.shape)) {
          throw new Error(`Unrecognized key: "${key}"`);
        }
        if (!mask[key])
          continue;
        delete newShape[key];
      }
      assignProp(this, "shape", newShape);
      return newShape;
    },
    checks: []
  });
  return clone(schema, def);
}
function extend(schema, shape) {
  if (!isPlainObject(shape)) {
    throw new Error("Invalid input to extend: expected a plain object");
  }
  const checks = schema._zod.def.checks;
  const hasChecks = checks && checks.length > 0;
  if (hasChecks) {
    throw new Error("Object schemas containing refinements cannot be extended. Use `.safeExtend()` instead.");
  }
  const def = mergeDefs(schema._zod.def, {
    get shape() {
      const _shape = { ...schema._zod.def.shape, ...shape };
      assignProp(this, "shape", _shape);
      return _shape;
    },
    checks: []
  });
  return clone(schema, def);
}
function safeExtend(schema, shape) {
  if (!isPlainObject(shape)) {
    throw new Error("Invalid input to safeExtend: expected a plain object");
  }
  const def = {
    ...schema._zod.def,
    get shape() {
      const _shape = { ...schema._zod.def.shape, ...shape };
      assignProp(this, "shape", _shape);
      return _shape;
    },
    checks: schema._zod.def.checks
  };
  return clone(schema, def);
}
function merge(a, b) {
  const def = mergeDefs(a._zod.def, {
    get shape() {
      const _shape = { ...a._zod.def.shape, ...b._zod.def.shape };
      assignProp(this, "shape", _shape);
      return _shape;
    },
    get catchall() {
      return b._zod.def.catchall;
    },
    checks: []
  });
  return clone(a, def);
}
function partial(Class, schema, mask) {
  const def = mergeDefs(schema._zod.def, {
    get shape() {
      const oldShape = schema._zod.def.shape;
      const shape = { ...oldShape };
      if (mask) {
        for (const key in mask) {
          if (!(key in oldShape)) {
            throw new Error(`Unrecognized key: "${key}"`);
          }
          if (!mask[key])
            continue;
          shape[key] = Class ? new Class({
            type: "optional",
            innerType: oldShape[key]
          }) : oldShape[key];
        }
      } else {
        for (const key in oldShape) {
          shape[key] = Class ? new Class({
            type: "optional",
            innerType: oldShape[key]
          }) : oldShape[key];
        }
      }
      assignProp(this, "shape", shape);
      return shape;
    },
    checks: []
  });
  return clone(schema, def);
}
function required(Class, schema, mask) {
  const def = mergeDefs(schema._zod.def, {
    get shape() {
      const oldShape = schema._zod.def.shape;
      const shape = { ...oldShape };
      if (mask) {
        for (const key in mask) {
          if (!(key in shape)) {
            throw new Error(`Unrecognized key: "${key}"`);
          }
          if (!mask[key])
            continue;
          shape[key] = new Class({
            type: "nonoptional",
            innerType: oldShape[key]
          });
        }
      } else {
        for (const key in oldShape) {
          shape[key] = new Class({
            type: "nonoptional",
            innerType: oldShape[key]
          });
        }
      }
      assignProp(this, "shape", shape);
      return shape;
    },
    checks: []
  });
  return clone(schema, def);
}
function aborted(x, startIndex = 0) {
  if (x.aborted === true)
    return true;
  for (let i = startIndex;i < x.issues.length; i++) {
    if (x.issues[i]?.continue !== true) {
      return true;
    }
  }
  return false;
}
function prefixIssues(path, issues) {
  return issues.map((iss) => {
    var _a;
    (_a = iss).path ?? (_a.path = []);
    iss.path.unshift(path);
    return iss;
  });
}
function unwrapMessage(message) {
  return typeof message === "string" ? message : message?.message;
}
function finalizeIssue(iss, ctx, config) {
  const full = { ...iss, path: iss.path ?? [] };
  if (!iss.message) {
    const message = unwrapMessage(iss.inst?._zod.def?.error?.(iss)) ?? unwrapMessage(ctx?.error?.(iss)) ?? unwrapMessage(config.customError?.(iss)) ?? unwrapMessage(config.localeError?.(iss)) ?? "Invalid input";
    full.message = message;
  }
  delete full.inst;
  delete full.continue;
  if (!ctx?.reportInput) {
    delete full.input;
  }
  return full;
}
function getLengthableOrigin(input) {
  if (Array.isArray(input))
    return "array";
  if (typeof input === "string")
    return "string";
  return "unknown";
}
function issue(...args) {
  const [iss, input, inst] = args;
  if (typeof iss === "string") {
    return {
      message: iss,
      code: "custom",
      input,
      inst
    };
  }
  return { ...iss };
}

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/errors.js
var initializer = (inst, def) => {
  inst.name = "$ZodError";
  Object.defineProperty(inst, "_zod", {
    value: inst._zod,
    enumerable: false
  });
  Object.defineProperty(inst, "issues", {
    value: def,
    enumerable: false
  });
  inst.message = JSON.stringify(def, jsonStringifyReplacer, 2);
  Object.defineProperty(inst, "toString", {
    value: () => inst.message,
    enumerable: false
  });
};
var $ZodError = $constructor("$ZodError", initializer);
var $ZodRealError = $constructor("$ZodError", initializer, { Parent: Error });
function flattenError(error, mapper = (issue) => issue.message) {
  const fieldErrors = {};
  const formErrors = [];
  for (const sub of error.issues) {
    if (sub.path.length > 0) {
      fieldErrors[sub.path[0]] = fieldErrors[sub.path[0]] || [];
      fieldErrors[sub.path[0]].push(mapper(sub));
    } else {
      formErrors.push(mapper(sub));
    }
  }
  return { formErrors, fieldErrors };
}
function formatError(error, _mapper) {
  const mapper = _mapper || function(issue) {
    return issue.message;
  };
  const fieldErrors = { _errors: [] };
  const processError = (error) => {
    for (const issue of error.issues) {
      if (issue.code === "invalid_union" && issue.errors.length) {
        issue.errors.map((issues) => processError({ issues }));
      } else if (issue.code === "invalid_key") {
        processError({ issues: issue.issues });
      } else if (issue.code === "invalid_element") {
        processError({ issues: issue.issues });
      } else if (issue.path.length === 0) {
        fieldErrors._errors.push(mapper(issue));
      } else {
        let curr = fieldErrors;
        let i = 0;
        while (i < issue.path.length) {
          const el = issue.path[i];
          const terminal = i === issue.path.length - 1;
          if (!terminal) {
            curr[el] = curr[el] || { _errors: [] };
          } else {
            curr[el] = curr[el] || { _errors: [] };
            curr[el]._errors.push(mapper(issue));
          }
          curr = curr[el];
          i++;
        }
      }
    }
  };
  processError(error);
  return fieldErrors;
}

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/parse.js
var _parse = (_Err) => (schema, value, _ctx, _params) => {
  const ctx = _ctx ? Object.assign(_ctx, { async: false }) : { async: false };
  const result = schema._zod.run({ value, issues: [] }, ctx);
  if (result instanceof Promise) {
    throw new $ZodAsyncError;
  }
  if (result.issues.length) {
    const e = new (_params?.Err ?? _Err)(result.issues.map((iss) => finalizeIssue(iss, ctx, config())));
    captureStackTrace(e, _params?.callee);
    throw e;
  }
  return result.value;
};
var _parseAsync = (_Err) => async (schema, value, _ctx, params) => {
  const ctx = _ctx ? Object.assign(_ctx, { async: true }) : { async: true };
  let result = schema._zod.run({ value, issues: [] }, ctx);
  if (result instanceof Promise)
    result = await result;
  if (result.issues.length) {
    const e = new (params?.Err ?? _Err)(result.issues.map((iss) => finalizeIssue(iss, ctx, config())));
    captureStackTrace(e, params?.callee);
    throw e;
  }
  return result.value;
};
var _safeParse = (_Err) => (schema, value, _ctx) => {
  const ctx = _ctx ? { ..._ctx, async: false } : { async: false };
  const result = schema._zod.run({ value, issues: [] }, ctx);
  if (result instanceof Promise) {
    throw new $ZodAsyncError;
  }
  return result.issues.length ? {
    success: false,
    error: new (_Err ?? $ZodError)(result.issues.map((iss) => finalizeIssue(iss, ctx, config())))
  } : { success: true, data: result.value };
};
var safeParse = /* @__PURE__ */ _safeParse($ZodRealError);
var _safeParseAsync = (_Err) => async (schema, value, _ctx) => {
  const ctx = _ctx ? Object.assign(_ctx, { async: true }) : { async: true };
  let result = schema._zod.run({ value, issues: [] }, ctx);
  if (result instanceof Promise)
    result = await result;
  return result.issues.length ? {
    success: false,
    error: new _Err(result.issues.map((iss) => finalizeIssue(iss, ctx, config())))
  } : { success: true, data: result.value };
};
var safeParseAsync = /* @__PURE__ */ _safeParseAsync($ZodRealError);
var _encode = (_Err) => (schema, value, _ctx) => {
  const ctx = _ctx ? Object.assign(_ctx, { direction: "backward" }) : { direction: "backward" };
  return _parse(_Err)(schema, value, ctx);
};
var _decode = (_Err) => (schema, value, _ctx) => {
  return _parse(_Err)(schema, value, _ctx);
};
var _encodeAsync = (_Err) => async (schema, value, _ctx) => {
  const ctx = _ctx ? Object.assign(_ctx, { direction: "backward" }) : { direction: "backward" };
  return _parseAsync(_Err)(schema, value, ctx);
};
var _decodeAsync = (_Err) => async (schema, value, _ctx) => {
  return _parseAsync(_Err)(schema, value, _ctx);
};
var _safeEncode = (_Err) => (schema, value, _ctx) => {
  const ctx = _ctx ? Object.assign(_ctx, { direction: "backward" }) : { direction: "backward" };
  return _safeParse(_Err)(schema, value, ctx);
};
var _safeDecode = (_Err) => (schema, value, _ctx) => {
  return _safeParse(_Err)(schema, value, _ctx);
};
var _safeEncodeAsync = (_Err) => async (schema, value, _ctx) => {
  const ctx = _ctx ? Object.assign(_ctx, { direction: "backward" }) : { direction: "backward" };
  return _safeParseAsync(_Err)(schema, value, ctx);
};
var _safeDecodeAsync = (_Err) => async (schema, value, _ctx) => {
  return _safeParseAsync(_Err)(schema, value, _ctx);
};
// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/regexes.js
var cuid = /^[cC][^\s-]{8,}$/;
var cuid2 = /^[0-9a-z]+$/;
var ulid = /^[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}$/;
var xid = /^[0-9a-vA-V]{20}$/;
var ksuid = /^[A-Za-z0-9]{27}$/;
var nanoid = /^[a-zA-Z0-9_-]{21}$/;
var duration = /^P(?:(\d+W)|(?!.*W)(?=\d|T\d)(\d+Y)?(\d+M)?(\d+D)?(T(?=\d)(\d+H)?(\d+M)?(\d+([.,]\d+)?S)?)?)$/;
var guid = /^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})$/;
var uuid = (version) => {
  if (!version)
    return /^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}|00000000-0000-0000-0000-000000000000|ffffffff-ffff-ffff-ffff-ffffffffffff)$/;
  return new RegExp(`^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-${version}[0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12})$`);
};
var email = /^(?!\.)(?!.*\.\.)([A-Za-z0-9_'+\-\.]*)[A-Za-z0-9_+-]@([A-Za-z0-9][A-Za-z0-9\-]*\.)+[A-Za-z]{2,}$/;
var _emoji = `^(\\p{Extended_Pictographic}|\\p{Emoji_Component})+$`;
function emoji() {
  return new RegExp(_emoji, "u");
}
var ipv4 = /^(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])$/;
var ipv6 = /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|::|([0-9a-fA-F]{1,4})?::([0-9a-fA-F]{1,4}:?){0,6})$/;
var cidrv4 = /^((25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\/([0-9]|[1-2][0-9]|3[0-2])$/;
var cidrv6 = /^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|::|([0-9a-fA-F]{1,4})?::([0-9a-fA-F]{1,4}:?){0,6})\/(12[0-8]|1[01][0-9]|[1-9]?[0-9])$/;
var base64 = /^$|^(?:[0-9a-zA-Z+/]{4})*(?:(?:[0-9a-zA-Z+/]{2}==)|(?:[0-9a-zA-Z+/]{3}=))?$/;
var base64url = /^[A-Za-z0-9_-]*$/;
var hostname = /^(?=.{1,253}\.?$)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[-0-9a-zA-Z]{0,61}[0-9a-zA-Z])?)*\.?$/;
var e164 = /^\+(?:[0-9]){6,14}[0-9]$/;
var dateSource = `(?:(?:\\d\\d[2468][048]|\\d\\d[13579][26]|\\d\\d0[48]|[02468][048]00|[13579][26]00)-02-29|\\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\\d|30)|(?:02)-(?:0[1-9]|1\\d|2[0-8])))`;
var date = /* @__PURE__ */ new RegExp(`^${dateSource}$`);
function timeSource(args) {
  const hhmm = `(?:[01]\\d|2[0-3]):[0-5]\\d`;
  const regex = typeof args.precision === "number" ? args.precision === -1 ? `${hhmm}` : args.precision === 0 ? `${hhmm}:[0-5]\\d` : `${hhmm}:[0-5]\\d\\.\\d{${args.precision}}` : `${hhmm}(?::[0-5]\\d(?:\\.\\d+)?)?`;
  return regex;
}
function time(args) {
  return new RegExp(`^${timeSource(args)}$`);
}
function datetime(args) {
  const time = timeSource({ precision: args.precision });
  const opts = ["Z"];
  if (args.local)
    opts.push("");
  if (args.offset)
    opts.push(`([+-](?:[01]\\d|2[0-3]):[0-5]\\d)`);
  const timeRegex = `${time}(?:${opts.join("|")})`;
  return new RegExp(`^${dateSource}T(?:${timeRegex})$`);
}
var string = (params) => {
  const regex = params ? `[\\s\\S]{${params?.minimum ?? 0},${params?.maximum ?? ""}}` : `[\\s\\S]*`;
  return new RegExp(`^${regex}$`);
};
var integer = /^\d+$/;
var number = /^-?\d+(?:\.\d+)?/i;
var boolean = /true|false/i;
var lowercase = /^[^A-Z]*$/;
var uppercase = /^[^a-z]*$/;

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/checks.js
var $ZodCheck = /* @__PURE__ */ $constructor("$ZodCheck", (inst, def) => {
  var _a;
  inst._zod ?? (inst._zod = {});
  inst._zod.def = def;
  (_a = inst._zod).onattach ?? (_a.onattach = []);
});
var numericOriginMap = {
  number: "number",
  bigint: "bigint",
  object: "date"
};
var $ZodCheckLessThan = /* @__PURE__ */ $constructor("$ZodCheckLessThan", (inst, def) => {
  $ZodCheck.init(inst, def);
  const origin = numericOriginMap[typeof def.value];
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    const curr = (def.inclusive ? bag.maximum : bag.exclusiveMaximum) ?? Number.POSITIVE_INFINITY;
    if (def.value < curr) {
      if (def.inclusive)
        bag.maximum = def.value;
      else
        bag.exclusiveMaximum = def.value;
    }
  });
  inst._zod.check = (payload) => {
    if (def.inclusive ? payload.value <= def.value : payload.value < def.value) {
      return;
    }
    payload.issues.push({
      origin,
      code: "too_big",
      maximum: def.value,
      input: payload.value,
      inclusive: def.inclusive,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckGreaterThan = /* @__PURE__ */ $constructor("$ZodCheckGreaterThan", (inst, def) => {
  $ZodCheck.init(inst, def);
  const origin = numericOriginMap[typeof def.value];
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    const curr = (def.inclusive ? bag.minimum : bag.exclusiveMinimum) ?? Number.NEGATIVE_INFINITY;
    if (def.value > curr) {
      if (def.inclusive)
        bag.minimum = def.value;
      else
        bag.exclusiveMinimum = def.value;
    }
  });
  inst._zod.check = (payload) => {
    if (def.inclusive ? payload.value >= def.value : payload.value > def.value) {
      return;
    }
    payload.issues.push({
      origin,
      code: "too_small",
      minimum: def.value,
      input: payload.value,
      inclusive: def.inclusive,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckMultipleOf = /* @__PURE__ */ $constructor("$ZodCheckMultipleOf", (inst, def) => {
  $ZodCheck.init(inst, def);
  inst._zod.onattach.push((inst) => {
    var _a;
    (_a = inst._zod.bag).multipleOf ?? (_a.multipleOf = def.value);
  });
  inst._zod.check = (payload) => {
    if (typeof payload.value !== typeof def.value)
      throw new Error("Cannot mix number and bigint in multiple_of check.");
    const isMultiple = typeof payload.value === "bigint" ? payload.value % def.value === BigInt(0) : floatSafeRemainder(payload.value, def.value) === 0;
    if (isMultiple)
      return;
    payload.issues.push({
      origin: typeof payload.value,
      code: "not_multiple_of",
      divisor: def.value,
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckNumberFormat = /* @__PURE__ */ $constructor("$ZodCheckNumberFormat", (inst, def) => {
  $ZodCheck.init(inst, def);
  def.format = def.format || "float64";
  const isInt = def.format?.includes("int");
  const origin = isInt ? "int" : "number";
  const [minimum, maximum] = NUMBER_FORMAT_RANGES[def.format];
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.format = def.format;
    bag.minimum = minimum;
    bag.maximum = maximum;
    if (isInt)
      bag.pattern = integer;
  });
  inst._zod.check = (payload) => {
    const input = payload.value;
    if (isInt) {
      if (!Number.isInteger(input)) {
        payload.issues.push({
          expected: origin,
          format: def.format,
          code: "invalid_type",
          continue: false,
          input,
          inst
        });
        return;
      }
      if (!Number.isSafeInteger(input)) {
        if (input > 0) {
          payload.issues.push({
            input,
            code: "too_big",
            maximum: Number.MAX_SAFE_INTEGER,
            note: "Integers must be within the safe integer range.",
            inst,
            origin,
            continue: !def.abort
          });
        } else {
          payload.issues.push({
            input,
            code: "too_small",
            minimum: Number.MIN_SAFE_INTEGER,
            note: "Integers must be within the safe integer range.",
            inst,
            origin,
            continue: !def.abort
          });
        }
        return;
      }
    }
    if (input < minimum) {
      payload.issues.push({
        origin: "number",
        input,
        code: "too_small",
        minimum,
        inclusive: true,
        inst,
        continue: !def.abort
      });
    }
    if (input > maximum) {
      payload.issues.push({
        origin: "number",
        input,
        code: "too_big",
        maximum,
        inst
      });
    }
  };
});
var $ZodCheckMaxLength = /* @__PURE__ */ $constructor("$ZodCheckMaxLength", (inst, def) => {
  var _a;
  $ZodCheck.init(inst, def);
  (_a = inst._zod.def).when ?? (_a.when = (payload) => {
    const val = payload.value;
    return !nullish(val) && val.length !== undefined;
  });
  inst._zod.onattach.push((inst) => {
    const curr = inst._zod.bag.maximum ?? Number.POSITIVE_INFINITY;
    if (def.maximum < curr)
      inst._zod.bag.maximum = def.maximum;
  });
  inst._zod.check = (payload) => {
    const input = payload.value;
    const length = input.length;
    if (length <= def.maximum)
      return;
    const origin = getLengthableOrigin(input);
    payload.issues.push({
      origin,
      code: "too_big",
      maximum: def.maximum,
      inclusive: true,
      input,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckMinLength = /* @__PURE__ */ $constructor("$ZodCheckMinLength", (inst, def) => {
  var _a;
  $ZodCheck.init(inst, def);
  (_a = inst._zod.def).when ?? (_a.when = (payload) => {
    const val = payload.value;
    return !nullish(val) && val.length !== undefined;
  });
  inst._zod.onattach.push((inst) => {
    const curr = inst._zod.bag.minimum ?? Number.NEGATIVE_INFINITY;
    if (def.minimum > curr)
      inst._zod.bag.minimum = def.minimum;
  });
  inst._zod.check = (payload) => {
    const input = payload.value;
    const length = input.length;
    if (length >= def.minimum)
      return;
    const origin = getLengthableOrigin(input);
    payload.issues.push({
      origin,
      code: "too_small",
      minimum: def.minimum,
      inclusive: true,
      input,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckLengthEquals = /* @__PURE__ */ $constructor("$ZodCheckLengthEquals", (inst, def) => {
  var _a;
  $ZodCheck.init(inst, def);
  (_a = inst._zod.def).when ?? (_a.when = (payload) => {
    const val = payload.value;
    return !nullish(val) && val.length !== undefined;
  });
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.minimum = def.length;
    bag.maximum = def.length;
    bag.length = def.length;
  });
  inst._zod.check = (payload) => {
    const input = payload.value;
    const length = input.length;
    if (length === def.length)
      return;
    const origin = getLengthableOrigin(input);
    const tooBig = length > def.length;
    payload.issues.push({
      origin,
      ...tooBig ? { code: "too_big", maximum: def.length } : { code: "too_small", minimum: def.length },
      inclusive: true,
      exact: true,
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckStringFormat = /* @__PURE__ */ $constructor("$ZodCheckStringFormat", (inst, def) => {
  var _a, _b;
  $ZodCheck.init(inst, def);
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.format = def.format;
    if (def.pattern) {
      bag.patterns ?? (bag.patterns = new Set);
      bag.patterns.add(def.pattern);
    }
  });
  if (def.pattern)
    (_a = inst._zod).check ?? (_a.check = (payload) => {
      def.pattern.lastIndex = 0;
      if (def.pattern.test(payload.value))
        return;
      payload.issues.push({
        origin: "string",
        code: "invalid_format",
        format: def.format,
        input: payload.value,
        ...def.pattern ? { pattern: def.pattern.toString() } : {},
        inst,
        continue: !def.abort
      });
    });
  else
    (_b = inst._zod).check ?? (_b.check = () => {});
});
var $ZodCheckRegex = /* @__PURE__ */ $constructor("$ZodCheckRegex", (inst, def) => {
  $ZodCheckStringFormat.init(inst, def);
  inst._zod.check = (payload) => {
    def.pattern.lastIndex = 0;
    if (def.pattern.test(payload.value))
      return;
    payload.issues.push({
      origin: "string",
      code: "invalid_format",
      format: "regex",
      input: payload.value,
      pattern: def.pattern.toString(),
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckLowerCase = /* @__PURE__ */ $constructor("$ZodCheckLowerCase", (inst, def) => {
  def.pattern ?? (def.pattern = lowercase);
  $ZodCheckStringFormat.init(inst, def);
});
var $ZodCheckUpperCase = /* @__PURE__ */ $constructor("$ZodCheckUpperCase", (inst, def) => {
  def.pattern ?? (def.pattern = uppercase);
  $ZodCheckStringFormat.init(inst, def);
});
var $ZodCheckIncludes = /* @__PURE__ */ $constructor("$ZodCheckIncludes", (inst, def) => {
  $ZodCheck.init(inst, def);
  const escapedRegex = escapeRegex(def.includes);
  const pattern = new RegExp(typeof def.position === "number" ? `^.{${def.position}}${escapedRegex}` : escapedRegex);
  def.pattern = pattern;
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.patterns ?? (bag.patterns = new Set);
    bag.patterns.add(pattern);
  });
  inst._zod.check = (payload) => {
    if (payload.value.includes(def.includes, def.position))
      return;
    payload.issues.push({
      origin: "string",
      code: "invalid_format",
      format: "includes",
      includes: def.includes,
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckStartsWith = /* @__PURE__ */ $constructor("$ZodCheckStartsWith", (inst, def) => {
  $ZodCheck.init(inst, def);
  const pattern = new RegExp(`^${escapeRegex(def.prefix)}.*`);
  def.pattern ?? (def.pattern = pattern);
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.patterns ?? (bag.patterns = new Set);
    bag.patterns.add(pattern);
  });
  inst._zod.check = (payload) => {
    if (payload.value.startsWith(def.prefix))
      return;
    payload.issues.push({
      origin: "string",
      code: "invalid_format",
      format: "starts_with",
      prefix: def.prefix,
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckEndsWith = /* @__PURE__ */ $constructor("$ZodCheckEndsWith", (inst, def) => {
  $ZodCheck.init(inst, def);
  const pattern = new RegExp(`.*${escapeRegex(def.suffix)}$`);
  def.pattern ?? (def.pattern = pattern);
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.patterns ?? (bag.patterns = new Set);
    bag.patterns.add(pattern);
  });
  inst._zod.check = (payload) => {
    if (payload.value.endsWith(def.suffix))
      return;
    payload.issues.push({
      origin: "string",
      code: "invalid_format",
      format: "ends_with",
      suffix: def.suffix,
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodCheckOverwrite = /* @__PURE__ */ $constructor("$ZodCheckOverwrite", (inst, def) => {
  $ZodCheck.init(inst, def);
  inst._zod.check = (payload) => {
    payload.value = def.tx(payload.value);
  };
});

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/doc.js
class Doc {
  constructor(args = []) {
    this.content = [];
    this.indent = 0;
    if (this)
      this.args = args;
  }
  indented(fn) {
    this.indent += 1;
    fn(this);
    this.indent -= 1;
  }
  write(arg) {
    if (typeof arg === "function") {
      arg(this, { execution: "sync" });
      arg(this, { execution: "async" });
      return;
    }
    const content = arg;
    const lines = content.split(`
`).filter((x) => x);
    const minIndent = Math.min(...lines.map((x) => x.length - x.trimStart().length));
    const dedented = lines.map((x) => x.slice(minIndent)).map((x) => " ".repeat(this.indent * 2) + x);
    for (const line of dedented) {
      this.content.push(line);
    }
  }
  compile() {
    const F = Function;
    const args = this?.args;
    const content = this?.content ?? [``];
    const lines = [...content.map((x) => `  ${x}`)];
    return new F(...args, lines.join(`
`));
  }
}

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/versions.js
var version = {
  major: 4,
  minor: 1,
  patch: 5
};

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/schemas.js
var $ZodType = /* @__PURE__ */ $constructor("$ZodType", (inst, def) => {
  var _a;
  inst ?? (inst = {});
  inst._zod.def = def;
  inst._zod.bag = inst._zod.bag || {};
  inst._zod.version = version;
  const checks = [...inst._zod.def.checks ?? []];
  if (inst._zod.traits.has("$ZodCheck")) {
    checks.unshift(inst);
  }
  for (const ch of checks) {
    for (const fn of ch._zod.onattach) {
      fn(inst);
    }
  }
  if (checks.length === 0) {
    (_a = inst._zod).deferred ?? (_a.deferred = []);
    inst._zod.deferred?.push(() => {
      inst._zod.run = inst._zod.parse;
    });
  } else {
    const runChecks = (payload, checks, ctx) => {
      let isAborted = aborted(payload);
      let asyncResult;
      for (const ch of checks) {
        if (ch._zod.def.when) {
          const shouldRun = ch._zod.def.when(payload);
          if (!shouldRun)
            continue;
        } else if (isAborted) {
          continue;
        }
        const currLen = payload.issues.length;
        const _ = ch._zod.check(payload);
        if (_ instanceof Promise && ctx?.async === false) {
          throw new $ZodAsyncError;
        }
        if (asyncResult || _ instanceof Promise) {
          asyncResult = (asyncResult ?? Promise.resolve()).then(async () => {
            await _;
            const nextLen = payload.issues.length;
            if (nextLen === currLen)
              return;
            if (!isAborted)
              isAborted = aborted(payload, currLen);
          });
        } else {
          const nextLen = payload.issues.length;
          if (nextLen === currLen)
            continue;
          if (!isAborted)
            isAborted = aborted(payload, currLen);
        }
      }
      if (asyncResult) {
        return asyncResult.then(() => {
          return payload;
        });
      }
      return payload;
    };
    const handleCanaryResult = (canary, payload, ctx) => {
      if (aborted(canary)) {
        canary.aborted = true;
        return canary;
      }
      const checkResult = runChecks(payload, checks, ctx);
      if (checkResult instanceof Promise) {
        if (ctx.async === false)
          throw new $ZodAsyncError;
        return checkResult.then((checkResult) => inst._zod.parse(checkResult, ctx));
      }
      return inst._zod.parse(checkResult, ctx);
    };
    inst._zod.run = (payload, ctx) => {
      if (ctx.skipChecks) {
        return inst._zod.parse(payload, ctx);
      }
      if (ctx.direction === "backward") {
        const canary = inst._zod.parse({ value: payload.value, issues: [] }, { ...ctx, skipChecks: true });
        if (canary instanceof Promise) {
          return canary.then((canary) => {
            return handleCanaryResult(canary, payload, ctx);
          });
        }
        return handleCanaryResult(canary, payload, ctx);
      }
      const result = inst._zod.parse(payload, ctx);
      if (result instanceof Promise) {
        if (ctx.async === false)
          throw new $ZodAsyncError;
        return result.then((result) => runChecks(result, checks, ctx));
      }
      return runChecks(result, checks, ctx);
    };
  }
  inst["~standard"] = {
    validate: (value) => {
      try {
        const r = safeParse(inst, value);
        return r.success ? { value: r.data } : { issues: r.error?.issues };
      } catch (_) {
        return safeParseAsync(inst, value).then((r) => r.success ? { value: r.data } : { issues: r.error?.issues });
      }
    },
    vendor: "zod",
    version: 1
  };
});
var $ZodString = /* @__PURE__ */ $constructor("$ZodString", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.pattern = [...inst?._zod.bag?.patterns ?? []].pop() ?? string(inst._zod.bag);
  inst._zod.parse = (payload, _) => {
    if (def.coerce)
      try {
        payload.value = String(payload.value);
      } catch (_) {}
    if (typeof payload.value === "string")
      return payload;
    payload.issues.push({
      expected: "string",
      code: "invalid_type",
      input: payload.value,
      inst
    });
    return payload;
  };
});
var $ZodStringFormat = /* @__PURE__ */ $constructor("$ZodStringFormat", (inst, def) => {
  $ZodCheckStringFormat.init(inst, def);
  $ZodString.init(inst, def);
});
var $ZodGUID = /* @__PURE__ */ $constructor("$ZodGUID", (inst, def) => {
  def.pattern ?? (def.pattern = guid);
  $ZodStringFormat.init(inst, def);
});
var $ZodUUID = /* @__PURE__ */ $constructor("$ZodUUID", (inst, def) => {
  if (def.version) {
    const versionMap = {
      v1: 1,
      v2: 2,
      v3: 3,
      v4: 4,
      v5: 5,
      v6: 6,
      v7: 7,
      v8: 8
    };
    const v = versionMap[def.version];
    if (v === undefined)
      throw new Error(`Invalid UUID version: "${def.version}"`);
    def.pattern ?? (def.pattern = uuid(v));
  } else
    def.pattern ?? (def.pattern = uuid());
  $ZodStringFormat.init(inst, def);
});
var $ZodEmail = /* @__PURE__ */ $constructor("$ZodEmail", (inst, def) => {
  def.pattern ?? (def.pattern = email);
  $ZodStringFormat.init(inst, def);
});
var $ZodURL = /* @__PURE__ */ $constructor("$ZodURL", (inst, def) => {
  $ZodStringFormat.init(inst, def);
  inst._zod.check = (payload) => {
    try {
      const trimmed = payload.value.trim();
      const url = new URL(trimmed);
      if (def.hostname) {
        def.hostname.lastIndex = 0;
        if (!def.hostname.test(url.hostname)) {
          payload.issues.push({
            code: "invalid_format",
            format: "url",
            note: "Invalid hostname",
            pattern: hostname.source,
            input: payload.value,
            inst,
            continue: !def.abort
          });
        }
      }
      if (def.protocol) {
        def.protocol.lastIndex = 0;
        if (!def.protocol.test(url.protocol.endsWith(":") ? url.protocol.slice(0, -1) : url.protocol)) {
          payload.issues.push({
            code: "invalid_format",
            format: "url",
            note: "Invalid protocol",
            pattern: def.protocol.source,
            input: payload.value,
            inst,
            continue: !def.abort
          });
        }
      }
      if (def.normalize) {
        payload.value = url.href;
      } else {
        payload.value = trimmed;
      }
      return;
    } catch (_) {
      payload.issues.push({
        code: "invalid_format",
        format: "url",
        input: payload.value,
        inst,
        continue: !def.abort
      });
    }
  };
});
var $ZodEmoji = /* @__PURE__ */ $constructor("$ZodEmoji", (inst, def) => {
  def.pattern ?? (def.pattern = emoji());
  $ZodStringFormat.init(inst, def);
});
var $ZodNanoID = /* @__PURE__ */ $constructor("$ZodNanoID", (inst, def) => {
  def.pattern ?? (def.pattern = nanoid);
  $ZodStringFormat.init(inst, def);
});
var $ZodCUID = /* @__PURE__ */ $constructor("$ZodCUID", (inst, def) => {
  def.pattern ?? (def.pattern = cuid);
  $ZodStringFormat.init(inst, def);
});
var $ZodCUID2 = /* @__PURE__ */ $constructor("$ZodCUID2", (inst, def) => {
  def.pattern ?? (def.pattern = cuid2);
  $ZodStringFormat.init(inst, def);
});
var $ZodULID = /* @__PURE__ */ $constructor("$ZodULID", (inst, def) => {
  def.pattern ?? (def.pattern = ulid);
  $ZodStringFormat.init(inst, def);
});
var $ZodXID = /* @__PURE__ */ $constructor("$ZodXID", (inst, def) => {
  def.pattern ?? (def.pattern = xid);
  $ZodStringFormat.init(inst, def);
});
var $ZodKSUID = /* @__PURE__ */ $constructor("$ZodKSUID", (inst, def) => {
  def.pattern ?? (def.pattern = ksuid);
  $ZodStringFormat.init(inst, def);
});
var $ZodISODateTime = /* @__PURE__ */ $constructor("$ZodISODateTime", (inst, def) => {
  def.pattern ?? (def.pattern = datetime(def));
  $ZodStringFormat.init(inst, def);
});
var $ZodISODate = /* @__PURE__ */ $constructor("$ZodISODate", (inst, def) => {
  def.pattern ?? (def.pattern = date);
  $ZodStringFormat.init(inst, def);
});
var $ZodISOTime = /* @__PURE__ */ $constructor("$ZodISOTime", (inst, def) => {
  def.pattern ?? (def.pattern = time(def));
  $ZodStringFormat.init(inst, def);
});
var $ZodISODuration = /* @__PURE__ */ $constructor("$ZodISODuration", (inst, def) => {
  def.pattern ?? (def.pattern = duration);
  $ZodStringFormat.init(inst, def);
});
var $ZodIPv4 = /* @__PURE__ */ $constructor("$ZodIPv4", (inst, def) => {
  def.pattern ?? (def.pattern = ipv4);
  $ZodStringFormat.init(inst, def);
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.format = `ipv4`;
  });
});
var $ZodIPv6 = /* @__PURE__ */ $constructor("$ZodIPv6", (inst, def) => {
  def.pattern ?? (def.pattern = ipv6);
  $ZodStringFormat.init(inst, def);
  inst._zod.onattach.push((inst) => {
    const bag = inst._zod.bag;
    bag.format = `ipv6`;
  });
  inst._zod.check = (payload) => {
    try {
      new URL(`http://[${payload.value}]`);
    } catch {
      payload.issues.push({
        code: "invalid_format",
        format: "ipv6",
        input: payload.value,
        inst,
        continue: !def.abort
      });
    }
  };
});
var $ZodCIDRv4 = /* @__PURE__ */ $constructor("$ZodCIDRv4", (inst, def) => {
  def.pattern ?? (def.pattern = cidrv4);
  $ZodStringFormat.init(inst, def);
});
var $ZodCIDRv6 = /* @__PURE__ */ $constructor("$ZodCIDRv6", (inst, def) => {
  def.pattern ?? (def.pattern = cidrv6);
  $ZodStringFormat.init(inst, def);
  inst._zod.check = (payload) => {
    const [address, prefix] = payload.value.split("/");
    try {
      if (!prefix)
        throw new Error;
      const prefixNum = Number(prefix);
      if (`${prefixNum}` !== prefix)
        throw new Error;
      if (prefixNum < 0 || prefixNum > 128)
        throw new Error;
      new URL(`http://[${address}]`);
    } catch {
      payload.issues.push({
        code: "invalid_format",
        format: "cidrv6",
        input: payload.value,
        inst,
        continue: !def.abort
      });
    }
  };
});
function isValidBase64(data) {
  if (data === "")
    return true;
  if (data.length % 4 !== 0)
    return false;
  try {
    atob(data);
    return true;
  } catch {
    return false;
  }
}
var $ZodBase64 = /* @__PURE__ */ $constructor("$ZodBase64", (inst, def) => {
  def.pattern ?? (def.pattern = base64);
  $ZodStringFormat.init(inst, def);
  inst._zod.onattach.push((inst) => {
    inst._zod.bag.contentEncoding = "base64";
  });
  inst._zod.check = (payload) => {
    if (isValidBase64(payload.value))
      return;
    payload.issues.push({
      code: "invalid_format",
      format: "base64",
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
function isValidBase64URL(data) {
  if (!base64url.test(data))
    return false;
  const base64 = data.replace(/[-_]/g, (c) => c === "-" ? "+" : "/");
  const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, "=");
  return isValidBase64(padded);
}
var $ZodBase64URL = /* @__PURE__ */ $constructor("$ZodBase64URL", (inst, def) => {
  def.pattern ?? (def.pattern = base64url);
  $ZodStringFormat.init(inst, def);
  inst._zod.onattach.push((inst) => {
    inst._zod.bag.contentEncoding = "base64url";
  });
  inst._zod.check = (payload) => {
    if (isValidBase64URL(payload.value))
      return;
    payload.issues.push({
      code: "invalid_format",
      format: "base64url",
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodE164 = /* @__PURE__ */ $constructor("$ZodE164", (inst, def) => {
  def.pattern ?? (def.pattern = e164);
  $ZodStringFormat.init(inst, def);
});
function isValidJWT(token, algorithm = null) {
  try {
    const tokensParts = token.split(".");
    if (tokensParts.length !== 3)
      return false;
    const [header] = tokensParts;
    if (!header)
      return false;
    const parsedHeader = JSON.parse(atob(header));
    if ("typ" in parsedHeader && parsedHeader?.typ !== "JWT")
      return false;
    if (!parsedHeader.alg)
      return false;
    if (algorithm && (!("alg" in parsedHeader) || parsedHeader.alg !== algorithm))
      return false;
    return true;
  } catch {
    return false;
  }
}
var $ZodJWT = /* @__PURE__ */ $constructor("$ZodJWT", (inst, def) => {
  $ZodStringFormat.init(inst, def);
  inst._zod.check = (payload) => {
    if (isValidJWT(payload.value, def.alg))
      return;
    payload.issues.push({
      code: "invalid_format",
      format: "jwt",
      input: payload.value,
      inst,
      continue: !def.abort
    });
  };
});
var $ZodNumber = /* @__PURE__ */ $constructor("$ZodNumber", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.pattern = inst._zod.bag.pattern ?? number;
  inst._zod.parse = (payload, _ctx) => {
    if (def.coerce)
      try {
        payload.value = Number(payload.value);
      } catch (_) {}
    const input = payload.value;
    if (typeof input === "number" && !Number.isNaN(input) && Number.isFinite(input)) {
      return payload;
    }
    const received = typeof input === "number" ? Number.isNaN(input) ? "NaN" : !Number.isFinite(input) ? "Infinity" : undefined : undefined;
    payload.issues.push({
      expected: "number",
      code: "invalid_type",
      input,
      inst,
      ...received ? { received } : {}
    });
    return payload;
  };
});
var $ZodNumberFormat = /* @__PURE__ */ $constructor("$ZodNumber", (inst, def) => {
  $ZodCheckNumberFormat.init(inst, def);
  $ZodNumber.init(inst, def);
});
var $ZodBoolean = /* @__PURE__ */ $constructor("$ZodBoolean", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.pattern = boolean;
  inst._zod.parse = (payload, _ctx) => {
    if (def.coerce)
      try {
        payload.value = Boolean(payload.value);
      } catch (_) {}
    const input = payload.value;
    if (typeof input === "boolean")
      return payload;
    payload.issues.push({
      expected: "boolean",
      code: "invalid_type",
      input,
      inst
    });
    return payload;
  };
});
var $ZodUnknown = /* @__PURE__ */ $constructor("$ZodUnknown", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.parse = (payload) => payload;
});
var $ZodNever = /* @__PURE__ */ $constructor("$ZodNever", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.parse = (payload, _ctx) => {
    payload.issues.push({
      expected: "never",
      code: "invalid_type",
      input: payload.value,
      inst
    });
    return payload;
  };
});
function handleArrayResult(result, final, index) {
  if (result.issues.length) {
    final.issues.push(...prefixIssues(index, result.issues));
  }
  final.value[index] = result.value;
}
var $ZodArray = /* @__PURE__ */ $constructor("$ZodArray", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.parse = (payload, ctx) => {
    const input = payload.value;
    if (!Array.isArray(input)) {
      payload.issues.push({
        expected: "array",
        code: "invalid_type",
        input,
        inst
      });
      return payload;
    }
    payload.value = Array(input.length);
    const proms = [];
    for (let i = 0;i < input.length; i++) {
      const item = input[i];
      const result = def.element._zod.run({
        value: item,
        issues: []
      }, ctx);
      if (result instanceof Promise) {
        proms.push(result.then((result) => handleArrayResult(result, payload, i)));
      } else {
        handleArrayResult(result, payload, i);
      }
    }
    if (proms.length) {
      return Promise.all(proms).then(() => payload);
    }
    return payload;
  };
});
function handlePropertyResult(result, final, key, input) {
  if (result.issues.length) {
    final.issues.push(...prefixIssues(key, result.issues));
  }
  if (result.value === undefined) {
    if (key in input) {
      final.value[key] = undefined;
    }
  } else {
    final.value[key] = result.value;
  }
}
function normalizeDef(def) {
  const keys = Object.keys(def.shape);
  for (const k of keys) {
    if (!def.shape[k]._zod.traits.has("$ZodType")) {
      throw new Error(`Invalid element at key "${k}": expected a Zod schema`);
    }
  }
  const okeys = optionalKeys(def.shape);
  return {
    ...def,
    keys,
    keySet: new Set(keys),
    numKeys: keys.length,
    optionalKeys: new Set(okeys)
  };
}
function handleCatchall(proms, input, payload, ctx, def, inst) {
  const unrecognized = [];
  const keySet = def.keySet;
  const _catchall = def.catchall._zod;
  const t = _catchall.def.type;
  for (const key of Object.keys(input)) {
    if (keySet.has(key))
      continue;
    if (t === "never") {
      unrecognized.push(key);
      continue;
    }
    const r = _catchall.run({ value: input[key], issues: [] }, ctx);
    if (r instanceof Promise) {
      proms.push(r.then((r) => handlePropertyResult(r, payload, key, input)));
    } else {
      handlePropertyResult(r, payload, key, input);
    }
  }
  if (unrecognized.length) {
    payload.issues.push({
      code: "unrecognized_keys",
      keys: unrecognized,
      input,
      inst
    });
  }
  if (!proms.length)
    return payload;
  return Promise.all(proms).then(() => {
    return payload;
  });
}
var $ZodObject = /* @__PURE__ */ $constructor("$ZodObject", (inst, def) => {
  $ZodType.init(inst, def);
  const _normalized = cached(() => normalizeDef(def));
  defineLazy(inst._zod, "propValues", () => {
    const shape = def.shape;
    const propValues = {};
    for (const key in shape) {
      const field = shape[key]._zod;
      if (field.values) {
        propValues[key] ?? (propValues[key] = new Set);
        for (const v of field.values)
          propValues[key].add(v);
      }
    }
    return propValues;
  });
  const isObject = isObject2;
  const catchall = def.catchall;
  let value;
  inst._zod.parse = (payload, ctx) => {
    value ?? (value = _normalized.value);
    const input = payload.value;
    if (!isObject(input)) {
      payload.issues.push({
        expected: "object",
        code: "invalid_type",
        input,
        inst
      });
      return payload;
    }
    payload.value = {};
    const proms = [];
    const shape = value.shape;
    for (const key of value.keys) {
      const el = shape[key];
      const r = el._zod.run({ value: input[key], issues: [] }, ctx);
      if (r instanceof Promise) {
        proms.push(r.then((r) => handlePropertyResult(r, payload, key, input)));
      } else {
        handlePropertyResult(r, payload, key, input);
      }
    }
    if (!catchall) {
      return proms.length ? Promise.all(proms).then(() => payload) : payload;
    }
    return handleCatchall(proms, input, payload, ctx, _normalized.value, inst);
  };
});
var $ZodObjectJIT = /* @__PURE__ */ $constructor("$ZodObjectJIT", (inst, def) => {
  $ZodObject.init(inst, def);
  const superParse = inst._zod.parse;
  const _normalized = cached(() => normalizeDef(def));
  const generateFastpass = (shape) => {
    const doc = new Doc(["shape", "payload", "ctx"]);
    const normalized = _normalized.value;
    const parseStr = (key) => {
      const k = esc(key);
      return `shape[${k}]._zod.run({ value: input[${k}], issues: [] }, ctx)`;
    };
    doc.write(`const input = payload.value;`);
    const ids = Object.create(null);
    let counter = 0;
    for (const key of normalized.keys) {
      ids[key] = `key_${counter++}`;
    }
    doc.write(`const newResult = {}`);
    for (const key of normalized.keys) {
      const id = ids[key];
      const k = esc(key);
      doc.write(`const ${id} = ${parseStr(key)};`);
      doc.write(`
        if (${id}.issues.length) {
          payload.issues = payload.issues.concat(${id}.issues.map(iss => ({
            ...iss,
            path: iss.path ? [${k}, ...iss.path] : [${k}]
          })));
        }
        
        if (${id}.value === undefined) {
          if (${k} in input) {
            newResult[${k}] = undefined;
          }
        } else {
          newResult[${k}] = ${id}.value;
        }
      `);
    }
    doc.write(`payload.value = newResult;`);
    doc.write(`return payload;`);
    const fn = doc.compile();
    return (payload, ctx) => fn(shape, payload, ctx);
  };
  let fastpass;
  const isObject = isObject2;
  const jit = !globalConfig.jitless;
  const allowsEval2 = allowsEval;
  const fastEnabled = jit && allowsEval2.value;
  const catchall = def.catchall;
  let value;
  inst._zod.parse = (payload, ctx) => {
    value ?? (value = _normalized.value);
    const input = payload.value;
    if (!isObject(input)) {
      payload.issues.push({
        expected: "object",
        code: "invalid_type",
        input,
        inst
      });
      return payload;
    }
    if (jit && fastEnabled && ctx?.async === false && ctx.jitless !== true) {
      if (!fastpass)
        fastpass = generateFastpass(def.shape);
      payload = fastpass(payload, ctx);
      if (!catchall)
        return payload;
      return handleCatchall([], input, payload, ctx, value, inst);
    }
    return superParse(payload, ctx);
  };
});
function handleUnionResults(results, final, inst, ctx) {
  for (const result of results) {
    if (result.issues.length === 0) {
      final.value = result.value;
      return final;
    }
  }
  const nonaborted = results.filter((r) => !aborted(r));
  if (nonaborted.length === 1) {
    final.value = nonaborted[0].value;
    return nonaborted[0];
  }
  final.issues.push({
    code: "invalid_union",
    input: final.value,
    inst,
    errors: results.map((result) => result.issues.map((iss) => finalizeIssue(iss, ctx, config())))
  });
  return final;
}
var $ZodUnion = /* @__PURE__ */ $constructor("$ZodUnion", (inst, def) => {
  $ZodType.init(inst, def);
  defineLazy(inst._zod, "optin", () => def.options.some((o) => o._zod.optin === "optional") ? "optional" : undefined);
  defineLazy(inst._zod, "optout", () => def.options.some((o) => o._zod.optout === "optional") ? "optional" : undefined);
  defineLazy(inst._zod, "values", () => {
    if (def.options.every((o) => o._zod.values)) {
      return new Set(def.options.flatMap((option) => Array.from(option._zod.values)));
    }
    return;
  });
  defineLazy(inst._zod, "pattern", () => {
    if (def.options.every((o) => o._zod.pattern)) {
      const patterns = def.options.map((o) => o._zod.pattern);
      return new RegExp(`^(${patterns.map((p) => cleanRegex(p.source)).join("|")})$`);
    }
    return;
  });
  const single = def.options.length === 1;
  const first = def.options[0]._zod.run;
  inst._zod.parse = (payload, ctx) => {
    if (single) {
      return first(payload, ctx);
    }
    let async = false;
    const results = [];
    for (const option of def.options) {
      const result = option._zod.run({
        value: payload.value,
        issues: []
      }, ctx);
      if (result instanceof Promise) {
        results.push(result);
        async = true;
      } else {
        if (result.issues.length === 0)
          return result;
        results.push(result);
      }
    }
    if (!async)
      return handleUnionResults(results, payload, inst, ctx);
    return Promise.all(results).then((results) => {
      return handleUnionResults(results, payload, inst, ctx);
    });
  };
});
var $ZodDiscriminatedUnion = /* @__PURE__ */ $constructor("$ZodDiscriminatedUnion", (inst, def) => {
  $ZodUnion.init(inst, def);
  const _super = inst._zod.parse;
  defineLazy(inst._zod, "propValues", () => {
    const propValues = {};
    for (const option of def.options) {
      const pv = option._zod.propValues;
      if (!pv || Object.keys(pv).length === 0)
        throw new Error(`Invalid discriminated union option at index "${def.options.indexOf(option)}"`);
      for (const [k, v] of Object.entries(pv)) {
        if (!propValues[k])
          propValues[k] = new Set;
        for (const val of v) {
          propValues[k].add(val);
        }
      }
    }
    return propValues;
  });
  const disc = cached(() => {
    const opts = def.options;
    const map = new Map;
    for (const o of opts) {
      const values = o._zod.propValues?.[def.discriminator];
      if (!values || values.size === 0)
        throw new Error(`Invalid discriminated union option at index "${def.options.indexOf(o)}"`);
      for (const v of values) {
        if (map.has(v)) {
          throw new Error(`Duplicate discriminator value "${String(v)}"`);
        }
        map.set(v, o);
      }
    }
    return map;
  });
  inst._zod.parse = (payload, ctx) => {
    const input = payload.value;
    if (!isObject2(input)) {
      payload.issues.push({
        code: "invalid_type",
        expected: "object",
        input,
        inst
      });
      return payload;
    }
    const opt = disc.value.get(input?.[def.discriminator]);
    if (opt) {
      return opt._zod.run(payload, ctx);
    }
    if (def.unionFallback) {
      return _super(payload, ctx);
    }
    payload.issues.push({
      code: "invalid_union",
      errors: [],
      note: "No matching discriminator",
      discriminator: def.discriminator,
      input,
      path: [def.discriminator],
      inst
    });
    return payload;
  };
});
var $ZodIntersection = /* @__PURE__ */ $constructor("$ZodIntersection", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.parse = (payload, ctx) => {
    const input = payload.value;
    const left = def.left._zod.run({ value: input, issues: [] }, ctx);
    const right = def.right._zod.run({ value: input, issues: [] }, ctx);
    const async = left instanceof Promise || right instanceof Promise;
    if (async) {
      return Promise.all([left, right]).then(([left, right]) => {
        return handleIntersectionResults(payload, left, right);
      });
    }
    return handleIntersectionResults(payload, left, right);
  };
});
function mergeValues(a, b) {
  if (a === b) {
    return { valid: true, data: a };
  }
  if (a instanceof Date && b instanceof Date && +a === +b) {
    return { valid: true, data: a };
  }
  if (isPlainObject(a) && isPlainObject(b)) {
    const bKeys = Object.keys(b);
    const sharedKeys = Object.keys(a).filter((key) => bKeys.indexOf(key) !== -1);
    const newObj = { ...a, ...b };
    for (const key of sharedKeys) {
      const sharedValue = mergeValues(a[key], b[key]);
      if (!sharedValue.valid) {
        return {
          valid: false,
          mergeErrorPath: [key, ...sharedValue.mergeErrorPath]
        };
      }
      newObj[key] = sharedValue.data;
    }
    return { valid: true, data: newObj };
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) {
      return { valid: false, mergeErrorPath: [] };
    }
    const newArray = [];
    for (let index = 0;index < a.length; index++) {
      const itemA = a[index];
      const itemB = b[index];
      const sharedValue = mergeValues(itemA, itemB);
      if (!sharedValue.valid) {
        return {
          valid: false,
          mergeErrorPath: [index, ...sharedValue.mergeErrorPath]
        };
      }
      newArray.push(sharedValue.data);
    }
    return { valid: true, data: newArray };
  }
  return { valid: false, mergeErrorPath: [] };
}
function handleIntersectionResults(result, left, right) {
  if (left.issues.length) {
    result.issues.push(...left.issues);
  }
  if (right.issues.length) {
    result.issues.push(...right.issues);
  }
  if (aborted(result))
    return result;
  const merged = mergeValues(left.value, right.value);
  if (!merged.valid) {
    throw new Error(`Unmergable intersection. Error path: ` + `${JSON.stringify(merged.mergeErrorPath)}`);
  }
  result.value = merged.data;
  return result;
}
var $ZodRecord = /* @__PURE__ */ $constructor("$ZodRecord", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.parse = (payload, ctx) => {
    const input = payload.value;
    if (!isPlainObject(input)) {
      payload.issues.push({
        expected: "record",
        code: "invalid_type",
        input,
        inst
      });
      return payload;
    }
    const proms = [];
    if (def.keyType._zod.values) {
      const values = def.keyType._zod.values;
      payload.value = {};
      for (const key of values) {
        if (typeof key === "string" || typeof key === "number" || typeof key === "symbol") {
          const result = def.valueType._zod.run({ value: input[key], issues: [] }, ctx);
          if (result instanceof Promise) {
            proms.push(result.then((result) => {
              if (result.issues.length) {
                payload.issues.push(...prefixIssues(key, result.issues));
              }
              payload.value[key] = result.value;
            }));
          } else {
            if (result.issues.length) {
              payload.issues.push(...prefixIssues(key, result.issues));
            }
            payload.value[key] = result.value;
          }
        }
      }
      let unrecognized;
      for (const key in input) {
        if (!values.has(key)) {
          unrecognized = unrecognized ?? [];
          unrecognized.push(key);
        }
      }
      if (unrecognized && unrecognized.length > 0) {
        payload.issues.push({
          code: "unrecognized_keys",
          input,
          inst,
          keys: unrecognized
        });
      }
    } else {
      payload.value = {};
      for (const key of Reflect.ownKeys(input)) {
        if (key === "__proto__")
          continue;
        const keyResult = def.keyType._zod.run({ value: key, issues: [] }, ctx);
        if (keyResult instanceof Promise) {
          throw new Error("Async schemas not supported in object keys currently");
        }
        if (keyResult.issues.length) {
          payload.issues.push({
            code: "invalid_key",
            origin: "record",
            issues: keyResult.issues.map((iss) => finalizeIssue(iss, ctx, config())),
            input: key,
            path: [key],
            inst
          });
          payload.value[keyResult.value] = keyResult.value;
          continue;
        }
        const result = def.valueType._zod.run({ value: input[key], issues: [] }, ctx);
        if (result instanceof Promise) {
          proms.push(result.then((result) => {
            if (result.issues.length) {
              payload.issues.push(...prefixIssues(key, result.issues));
            }
            payload.value[keyResult.value] = result.value;
          }));
        } else {
          if (result.issues.length) {
            payload.issues.push(...prefixIssues(key, result.issues));
          }
          payload.value[keyResult.value] = result.value;
        }
      }
    }
    if (proms.length) {
      return Promise.all(proms).then(() => payload);
    }
    return payload;
  };
});
var $ZodEnum = /* @__PURE__ */ $constructor("$ZodEnum", (inst, def) => {
  $ZodType.init(inst, def);
  const values = getEnumValues(def.entries);
  const valuesSet = new Set(values);
  inst._zod.values = valuesSet;
  inst._zod.pattern = new RegExp(`^(${values.filter((k) => propertyKeyTypes.has(typeof k)).map((o) => typeof o === "string" ? escapeRegex(o) : o.toString()).join("|")})$`);
  inst._zod.parse = (payload, _ctx) => {
    const input = payload.value;
    if (valuesSet.has(input)) {
      return payload;
    }
    payload.issues.push({
      code: "invalid_value",
      values,
      input,
      inst
    });
    return payload;
  };
});
var $ZodLiteral = /* @__PURE__ */ $constructor("$ZodLiteral", (inst, def) => {
  $ZodType.init(inst, def);
  if (def.values.length === 0) {
    throw new Error("Cannot create literal schema with no valid values");
  }
  inst._zod.values = new Set(def.values);
  inst._zod.pattern = new RegExp(`^(${def.values.map((o) => typeof o === "string" ? escapeRegex(o) : o ? escapeRegex(o.toString()) : String(o)).join("|")})$`);
  inst._zod.parse = (payload, _ctx) => {
    const input = payload.value;
    if (inst._zod.values.has(input)) {
      return payload;
    }
    payload.issues.push({
      code: "invalid_value",
      values: def.values,
      input,
      inst
    });
    return payload;
  };
});
var $ZodTransform = /* @__PURE__ */ $constructor("$ZodTransform", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.parse = (payload, ctx) => {
    if (ctx.direction === "backward") {
      throw new $ZodEncodeError(inst.constructor.name);
    }
    const _out = def.transform(payload.value, payload);
    if (ctx.async) {
      const output = _out instanceof Promise ? _out : Promise.resolve(_out);
      return output.then((output) => {
        payload.value = output;
        return payload;
      });
    }
    if (_out instanceof Promise) {
      throw new $ZodAsyncError;
    }
    payload.value = _out;
    return payload;
  };
});
function handleOptionalResult(result, input) {
  if (result.issues.length && input === undefined) {
    return { issues: [], value: undefined };
  }
  return result;
}
var $ZodOptional = /* @__PURE__ */ $constructor("$ZodOptional", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.optin = "optional";
  inst._zod.optout = "optional";
  defineLazy(inst._zod, "values", () => {
    return def.innerType._zod.values ? new Set([...def.innerType._zod.values, undefined]) : undefined;
  });
  defineLazy(inst._zod, "pattern", () => {
    const pattern = def.innerType._zod.pattern;
    return pattern ? new RegExp(`^(${cleanRegex(pattern.source)})?$`) : undefined;
  });
  inst._zod.parse = (payload, ctx) => {
    if (def.innerType._zod.optin === "optional") {
      const result = def.innerType._zod.run(payload, ctx);
      if (result instanceof Promise)
        return result.then((r) => handleOptionalResult(r, payload.value));
      return handleOptionalResult(result, payload.value);
    }
    if (payload.value === undefined) {
      return payload;
    }
    return def.innerType._zod.run(payload, ctx);
  };
});
var $ZodNullable = /* @__PURE__ */ $constructor("$ZodNullable", (inst, def) => {
  $ZodType.init(inst, def);
  defineLazy(inst._zod, "optin", () => def.innerType._zod.optin);
  defineLazy(inst._zod, "optout", () => def.innerType._zod.optout);
  defineLazy(inst._zod, "pattern", () => {
    const pattern = def.innerType._zod.pattern;
    return pattern ? new RegExp(`^(${cleanRegex(pattern.source)}|null)$`) : undefined;
  });
  defineLazy(inst._zod, "values", () => {
    return def.innerType._zod.values ? new Set([...def.innerType._zod.values, null]) : undefined;
  });
  inst._zod.parse = (payload, ctx) => {
    if (payload.value === null)
      return payload;
    return def.innerType._zod.run(payload, ctx);
  };
});
var $ZodDefault = /* @__PURE__ */ $constructor("$ZodDefault", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.optin = "optional";
  defineLazy(inst._zod, "values", () => def.innerType._zod.values);
  inst._zod.parse = (payload, ctx) => {
    if (ctx.direction === "backward") {
      return def.innerType._zod.run(payload, ctx);
    }
    if (payload.value === undefined) {
      payload.value = def.defaultValue;
      return payload;
    }
    const result = def.innerType._zod.run(payload, ctx);
    if (result instanceof Promise) {
      return result.then((result) => handleDefaultResult(result, def));
    }
    return handleDefaultResult(result, def);
  };
});
function handleDefaultResult(payload, def) {
  if (payload.value === undefined) {
    payload.value = def.defaultValue;
  }
  return payload;
}
var $ZodPrefault = /* @__PURE__ */ $constructor("$ZodPrefault", (inst, def) => {
  $ZodType.init(inst, def);
  inst._zod.optin = "optional";
  defineLazy(inst._zod, "values", () => def.innerType._zod.values);
  inst._zod.parse = (payload, ctx) => {
    if (ctx.direction === "backward") {
      return def.innerType._zod.run(payload, ctx);
    }
    if (payload.value === undefined) {
      payload.value = def.defaultValue;
    }
    return def.innerType._zod.run(payload, ctx);
  };
});
var $ZodNonOptional = /* @__PURE__ */ $constructor("$ZodNonOptional", (inst, def) => {
  $ZodType.init(inst, def);
  defineLazy(inst._zod, "values", () => {
    const v = def.innerType._zod.values;
    return v ? new Set([...v].filter((x) => x !== undefined)) : undefined;
  });
  inst._zod.parse = (payload, ctx) => {
    const result = def.innerType._zod.run(payload, ctx);
    if (result instanceof Promise) {
      return result.then((result) => handleNonOptionalResult(result, inst));
    }
    return handleNonOptionalResult(result, inst);
  };
});
function handleNonOptionalResult(payload, inst) {
  if (!payload.issues.length && payload.value === undefined) {
    payload.issues.push({
      code: "invalid_type",
      expected: "nonoptional",
      input: payload.value,
      inst
    });
  }
  return payload;
}
var $ZodCatch = /* @__PURE__ */ $constructor("$ZodCatch", (inst, def) => {
  $ZodType.init(inst, def);
  defineLazy(inst._zod, "optin", () => def.innerType._zod.optin);
  defineLazy(inst._zod, "optout", () => def.innerType._zod.optout);
  defineLazy(inst._zod, "values", () => def.innerType._zod.values);
  inst._zod.parse = (payload, ctx) => {
    if (ctx.direction === "backward") {
      return def.innerType._zod.run(payload, ctx);
    }
    const result = def.innerType._zod.run(payload, ctx);
    if (result instanceof Promise) {
      return result.then((result) => {
        payload.value = result.value;
        if (result.issues.length) {
          payload.value = def.catchValue({
            ...payload,
            error: {
              issues: result.issues.map((iss) => finalizeIssue(iss, ctx, config()))
            },
            input: payload.value
          });
          payload.issues = [];
        }
        return payload;
      });
    }
    payload.value = result.value;
    if (result.issues.length) {
      payload.value = def.catchValue({
        ...payload,
        error: {
          issues: result.issues.map((iss) => finalizeIssue(iss, ctx, config()))
        },
        input: payload.value
      });
      payload.issues = [];
    }
    return payload;
  };
});
var $ZodPipe = /* @__PURE__ */ $constructor("$ZodPipe", (inst, def) => {
  $ZodType.init(inst, def);
  defineLazy(inst._zod, "values", () => def.in._zod.values);
  defineLazy(inst._zod, "optin", () => def.in._zod.optin);
  defineLazy(inst._zod, "optout", () => def.out._zod.optout);
  defineLazy(inst._zod, "propValues", () => def.in._zod.propValues);
  inst._zod.parse = (payload, ctx) => {
    if (ctx.direction === "backward") {
      const right = def.out._zod.run(payload, ctx);
      if (right instanceof Promise) {
        return right.then((right) => handlePipeResult(right, def.in, ctx));
      }
      return handlePipeResult(right, def.in, ctx);
    }
    const left = def.in._zod.run(payload, ctx);
    if (left instanceof Promise) {
      return left.then((left) => handlePipeResult(left, def.out, ctx));
    }
    return handlePipeResult(left, def.out, ctx);
  };
});
function handlePipeResult(left, next, ctx) {
  if (left.issues.length) {
    left.aborted = true;
    return left;
  }
  return next._zod.run({ value: left.value, issues: left.issues }, ctx);
}
var $ZodReadonly = /* @__PURE__ */ $constructor("$ZodReadonly", (inst, def) => {
  $ZodType.init(inst, def);
  defineLazy(inst._zod, "propValues", () => def.innerType._zod.propValues);
  defineLazy(inst._zod, "values", () => def.innerType._zod.values);
  defineLazy(inst._zod, "optin", () => def.innerType._zod.optin);
  defineLazy(inst._zod, "optout", () => def.innerType._zod.optout);
  inst._zod.parse = (payload, ctx) => {
    if (ctx.direction === "backward") {
      return def.innerType._zod.run(payload, ctx);
    }
    const result = def.innerType._zod.run(payload, ctx);
    if (result instanceof Promise) {
      return result.then(handleReadonlyResult);
    }
    return handleReadonlyResult(result);
  };
});
function handleReadonlyResult(payload) {
  payload.value = Object.freeze(payload.value);
  return payload;
}
var $ZodCustom = /* @__PURE__ */ $constructor("$ZodCustom", (inst, def) => {
  $ZodCheck.init(inst, def);
  $ZodType.init(inst, def);
  inst._zod.parse = (payload, _) => {
    return payload;
  };
  inst._zod.check = (payload) => {
    const input = payload.value;
    const r = def.fn(input);
    if (r instanceof Promise) {
      return r.then((r) => handleRefineResult(r, payload, input, inst));
    }
    handleRefineResult(r, payload, input, inst);
    return;
  };
});
function handleRefineResult(result, payload, input, inst) {
  if (!result) {
    const _iss = {
      code: "custom",
      input,
      inst,
      path: [...inst._zod.def.path ?? []],
      continue: !inst._zod.def.abort
    };
    if (inst._zod.def.params)
      _iss.params = inst._zod.def.params;
    payload.issues.push(issue(_iss));
  }
}
// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/registries.js
var $output = Symbol("ZodOutput");
var $input = Symbol("ZodInput");

class $ZodRegistry {
  constructor() {
    this._map = new Map;
    this._idmap = new Map;
  }
  add(schema, ..._meta) {
    const meta = _meta[0];
    this._map.set(schema, meta);
    if (meta && typeof meta === "object" && "id" in meta) {
      if (this._idmap.has(meta.id)) {
        throw new Error(`ID ${meta.id} already exists in the registry`);
      }
      this._idmap.set(meta.id, schema);
    }
    return this;
  }
  clear() {
    this._map = new Map;
    this._idmap = new Map;
    return this;
  }
  remove(schema) {
    const meta = this._map.get(schema);
    if (meta && typeof meta === "object" && "id" in meta) {
      this._idmap.delete(meta.id);
    }
    this._map.delete(schema);
    return this;
  }
  get(schema) {
    const p = schema._zod.parent;
    if (p) {
      const pm = { ...this.get(p) ?? {} };
      delete pm.id;
      const f = { ...pm, ...this._map.get(schema) };
      return Object.keys(f).length ? f : undefined;
    }
    return this._map.get(schema);
  }
  has(schema) {
    return this._map.has(schema);
  }
}
function registry() {
  return new $ZodRegistry;
}
var globalRegistry = /* @__PURE__ */ registry();
// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/core/api.js
function _string(Class, params) {
  return new Class({
    type: "string",
    ...normalizeParams(params)
  });
}
function _email(Class, params) {
  return new Class({
    type: "string",
    format: "email",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _guid(Class, params) {
  return new Class({
    type: "string",
    format: "guid",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _uuid(Class, params) {
  return new Class({
    type: "string",
    format: "uuid",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _uuidv4(Class, params) {
  return new Class({
    type: "string",
    format: "uuid",
    check: "string_format",
    abort: false,
    version: "v4",
    ...normalizeParams(params)
  });
}
function _uuidv6(Class, params) {
  return new Class({
    type: "string",
    format: "uuid",
    check: "string_format",
    abort: false,
    version: "v6",
    ...normalizeParams(params)
  });
}
function _uuidv7(Class, params) {
  return new Class({
    type: "string",
    format: "uuid",
    check: "string_format",
    abort: false,
    version: "v7",
    ...normalizeParams(params)
  });
}
function _url(Class, params) {
  return new Class({
    type: "string",
    format: "url",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _emoji2(Class, params) {
  return new Class({
    type: "string",
    format: "emoji",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _nanoid(Class, params) {
  return new Class({
    type: "string",
    format: "nanoid",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _cuid(Class, params) {
  return new Class({
    type: "string",
    format: "cuid",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _cuid2(Class, params) {
  return new Class({
    type: "string",
    format: "cuid2",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _ulid(Class, params) {
  return new Class({
    type: "string",
    format: "ulid",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _xid(Class, params) {
  return new Class({
    type: "string",
    format: "xid",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _ksuid(Class, params) {
  return new Class({
    type: "string",
    format: "ksuid",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _ipv4(Class, params) {
  return new Class({
    type: "string",
    format: "ipv4",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _ipv6(Class, params) {
  return new Class({
    type: "string",
    format: "ipv6",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _cidrv4(Class, params) {
  return new Class({
    type: "string",
    format: "cidrv4",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _cidrv6(Class, params) {
  return new Class({
    type: "string",
    format: "cidrv6",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _base64(Class, params) {
  return new Class({
    type: "string",
    format: "base64",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _base64url(Class, params) {
  return new Class({
    type: "string",
    format: "base64url",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _e164(Class, params) {
  return new Class({
    type: "string",
    format: "e164",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _jwt(Class, params) {
  return new Class({
    type: "string",
    format: "jwt",
    check: "string_format",
    abort: false,
    ...normalizeParams(params)
  });
}
function _isoDateTime(Class, params) {
  return new Class({
    type: "string",
    format: "datetime",
    check: "string_format",
    offset: false,
    local: false,
    precision: null,
    ...normalizeParams(params)
  });
}
function _isoDate(Class, params) {
  return new Class({
    type: "string",
    format: "date",
    check: "string_format",
    ...normalizeParams(params)
  });
}
function _isoTime(Class, params) {
  return new Class({
    type: "string",
    format: "time",
    check: "string_format",
    precision: null,
    ...normalizeParams(params)
  });
}
function _isoDuration(Class, params) {
  return new Class({
    type: "string",
    format: "duration",
    check: "string_format",
    ...normalizeParams(params)
  });
}
function _number(Class, params) {
  return new Class({
    type: "number",
    checks: [],
    ...normalizeParams(params)
  });
}
function _int(Class, params) {
  return new Class({
    type: "number",
    check: "number_format",
    abort: false,
    format: "safeint",
    ...normalizeParams(params)
  });
}
function _boolean(Class, params) {
  return new Class({
    type: "boolean",
    ...normalizeParams(params)
  });
}
function _unknown(Class) {
  return new Class({
    type: "unknown"
  });
}
function _never(Class, params) {
  return new Class({
    type: "never",
    ...normalizeParams(params)
  });
}
function _lt(value, params) {
  return new $ZodCheckLessThan({
    check: "less_than",
    ...normalizeParams(params),
    value,
    inclusive: false
  });
}
function _lte(value, params) {
  return new $ZodCheckLessThan({
    check: "less_than",
    ...normalizeParams(params),
    value,
    inclusive: true
  });
}
function _gt(value, params) {
  return new $ZodCheckGreaterThan({
    check: "greater_than",
    ...normalizeParams(params),
    value,
    inclusive: false
  });
}
function _gte(value, params) {
  return new $ZodCheckGreaterThan({
    check: "greater_than",
    ...normalizeParams(params),
    value,
    inclusive: true
  });
}
function _multipleOf(value, params) {
  return new $ZodCheckMultipleOf({
    check: "multiple_of",
    ...normalizeParams(params),
    value
  });
}
function _maxLength(maximum, params) {
  const ch = new $ZodCheckMaxLength({
    check: "max_length",
    ...normalizeParams(params),
    maximum
  });
  return ch;
}
function _minLength(minimum, params) {
  return new $ZodCheckMinLength({
    check: "min_length",
    ...normalizeParams(params),
    minimum
  });
}
function _length(length, params) {
  return new $ZodCheckLengthEquals({
    check: "length_equals",
    ...normalizeParams(params),
    length
  });
}
function _regex(pattern, params) {
  return new $ZodCheckRegex({
    check: "string_format",
    format: "regex",
    ...normalizeParams(params),
    pattern
  });
}
function _lowercase(params) {
  return new $ZodCheckLowerCase({
    check: "string_format",
    format: "lowercase",
    ...normalizeParams(params)
  });
}
function _uppercase(params) {
  return new $ZodCheckUpperCase({
    check: "string_format",
    format: "uppercase",
    ...normalizeParams(params)
  });
}
function _includes(includes, params) {
  return new $ZodCheckIncludes({
    check: "string_format",
    format: "includes",
    ...normalizeParams(params),
    includes
  });
}
function _startsWith(prefix, params) {
  return new $ZodCheckStartsWith({
    check: "string_format",
    format: "starts_with",
    ...normalizeParams(params),
    prefix
  });
}
function _endsWith(suffix, params) {
  return new $ZodCheckEndsWith({
    check: "string_format",
    format: "ends_with",
    ...normalizeParams(params),
    suffix
  });
}
function _overwrite(tx) {
  return new $ZodCheckOverwrite({
    check: "overwrite",
    tx
  });
}
function _normalize(form) {
  return _overwrite((input) => input.normalize(form));
}
function _trim() {
  return _overwrite((input) => input.trim());
}
function _toLowerCase() {
  return _overwrite((input) => input.toLowerCase());
}
function _toUpperCase() {
  return _overwrite((input) => input.toUpperCase());
}
function _array(Class, element, params) {
  return new Class({
    type: "array",
    element,
    ...normalizeParams(params)
  });
}
function _refine(Class, fn, _params) {
  const schema = new Class({
    type: "custom",
    check: "custom",
    fn,
    ...normalizeParams(_params)
  });
  return schema;
}
function _superRefine(fn) {
  const ch = _check((payload) => {
    payload.addIssue = (issue2) => {
      if (typeof issue2 === "string") {
        payload.issues.push(issue(issue2, payload.value, ch._zod.def));
      } else {
        const _issue = issue2;
        if (_issue.fatal)
          _issue.continue = false;
        _issue.code ?? (_issue.code = "custom");
        _issue.input ?? (_issue.input = payload.value);
        _issue.inst ?? (_issue.inst = ch);
        _issue.continue ?? (_issue.continue = !ch._zod.def.abort);
        payload.issues.push(issue(_issue));
      }
    };
    return fn(payload.value, payload);
  });
  return ch;
}
function _check(fn, params) {
  const ch = new $ZodCheck({
    check: "custom",
    ...normalizeParams(params)
  });
  ch._zod.check = fn;
  return ch;
}
// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/classic/iso.js
var ZodISODateTime = /* @__PURE__ */ $constructor("ZodISODateTime", (inst, def) => {
  $ZodISODateTime.init(inst, def);
  ZodStringFormat.init(inst, def);
});
function datetime2(params) {
  return _isoDateTime(ZodISODateTime, params);
}
var ZodISODate = /* @__PURE__ */ $constructor("ZodISODate", (inst, def) => {
  $ZodISODate.init(inst, def);
  ZodStringFormat.init(inst, def);
});
function date2(params) {
  return _isoDate(ZodISODate, params);
}
var ZodISOTime = /* @__PURE__ */ $constructor("ZodISOTime", (inst, def) => {
  $ZodISOTime.init(inst, def);
  ZodStringFormat.init(inst, def);
});
function time2(params) {
  return _isoTime(ZodISOTime, params);
}
var ZodISODuration = /* @__PURE__ */ $constructor("ZodISODuration", (inst, def) => {
  $ZodISODuration.init(inst, def);
  ZodStringFormat.init(inst, def);
});
function duration2(params) {
  return _isoDuration(ZodISODuration, params);
}

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/classic/errors.js
var initializer2 = (inst, issues) => {
  $ZodError.init(inst, issues);
  inst.name = "ZodError";
  Object.defineProperties(inst, {
    format: {
      value: (mapper) => formatError(inst, mapper)
    },
    flatten: {
      value: (mapper) => flattenError(inst, mapper)
    },
    addIssue: {
      value: (issue) => {
        inst.issues.push(issue);
        inst.message = JSON.stringify(inst.issues, jsonStringifyReplacer, 2);
      }
    },
    addIssues: {
      value: (issues) => {
        inst.issues.push(...issues);
        inst.message = JSON.stringify(inst.issues, jsonStringifyReplacer, 2);
      }
    },
    isEmpty: {
      get() {
        return inst.issues.length === 0;
      }
    }
  });
};
var ZodError = $constructor("ZodError", initializer2);
var ZodRealError = $constructor("ZodError", initializer2, {
  Parent: Error
});

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/classic/parse.js
var parse3 = /* @__PURE__ */ _parse(ZodRealError);
var parseAsync2 = /* @__PURE__ */ _parseAsync(ZodRealError);
var safeParse2 = /* @__PURE__ */ _safeParse(ZodRealError);
var safeParseAsync2 = /* @__PURE__ */ _safeParseAsync(ZodRealError);
var encode = /* @__PURE__ */ _encode(ZodRealError);
var decode = /* @__PURE__ */ _decode(ZodRealError);
var encodeAsync = /* @__PURE__ */ _encodeAsync(ZodRealError);
var decodeAsync = /* @__PURE__ */ _decodeAsync(ZodRealError);
var safeEncode = /* @__PURE__ */ _safeEncode(ZodRealError);
var safeDecode = /* @__PURE__ */ _safeDecode(ZodRealError);
var safeEncodeAsync = /* @__PURE__ */ _safeEncodeAsync(ZodRealError);
var safeDecodeAsync = /* @__PURE__ */ _safeDecodeAsync(ZodRealError);

// node_modules/.bun/zod@4.1.5/node_modules/zod/v4/classic/schemas.js
var ZodType = /* @__PURE__ */ $constructor("ZodType", (inst, def) => {
  $ZodType.init(inst, def);
  inst.def = def;
  inst.type = def.type;
  Object.defineProperty(inst, "_def", { value: def });
  inst.check = (...checks) => {
    return inst.clone({
      ...def,
      checks: [
        ...def.checks ?? [],
        ...checks.map((ch) => typeof ch === "function" ? { _zod: { check: ch, def: { check: "custom" }, onattach: [] } } : ch)
      ]
    });
  };
  inst.clone = (def, params) => clone(inst, def, params);
  inst.brand = () => inst;
  inst.register = (reg, meta) => {
    reg.add(inst, meta);
    return inst;
  };
  inst.parse = (data, params) => parse3(inst, data, params, { callee: inst.parse });
  inst.safeParse = (data, params) => safeParse2(inst, data, params);
  inst.parseAsync = async (data, params) => parseAsync2(inst, data, params, { callee: inst.parseAsync });
  inst.safeParseAsync = async (data, params) => safeParseAsync2(inst, data, params);
  inst.spa = inst.safeParseAsync;
  inst.encode = (data, params) => encode(inst, data, params);
  inst.decode = (data, params) => decode(inst, data, params);
  inst.encodeAsync = async (data, params) => encodeAsync(inst, data, params);
  inst.decodeAsync = async (data, params) => decodeAsync(inst, data, params);
  inst.safeEncode = (data, params) => safeEncode(inst, data, params);
  inst.safeDecode = (data, params) => safeDecode(inst, data, params);
  inst.safeEncodeAsync = async (data, params) => safeEncodeAsync(inst, data, params);
  inst.safeDecodeAsync = async (data, params) => safeDecodeAsync(inst, data, params);
  inst.refine = (check, params) => inst.check(refine(check, params));
  inst.superRefine = (refinement) => inst.check(superRefine(refinement));
  inst.overwrite = (fn) => inst.check(_overwrite(fn));
  inst.optional = () => optional(inst);
  inst.nullable = () => nullable(inst);
  inst.nullish = () => optional(nullable(inst));
  inst.nonoptional = (params) => nonoptional(inst, params);
  inst.array = () => array(inst);
  inst.or = (arg) => union([inst, arg]);
  inst.and = (arg) => intersection(inst, arg);
  inst.transform = (tx) => pipe(inst, transform(tx));
  inst.default = (def) => _default(inst, def);
  inst.prefault = (def) => prefault(inst, def);
  inst.catch = (params) => _catch(inst, params);
  inst.pipe = (target) => pipe(inst, target);
  inst.readonly = () => readonly(inst);
  inst.describe = (description) => {
    const cl = inst.clone();
    globalRegistry.add(cl, { description });
    return cl;
  };
  Object.defineProperty(inst, "description", {
    get() {
      return globalRegistry.get(inst)?.description;
    },
    configurable: true
  });
  inst.meta = (...args) => {
    if (args.length === 0) {
      return globalRegistry.get(inst);
    }
    const cl = inst.clone();
    globalRegistry.add(cl, args[0]);
    return cl;
  };
  inst.isOptional = () => inst.safeParse(undefined).success;
  inst.isNullable = () => inst.safeParse(null).success;
  return inst;
});
var _ZodString = /* @__PURE__ */ $constructor("_ZodString", (inst, def) => {
  $ZodString.init(inst, def);
  ZodType.init(inst, def);
  const bag = inst._zod.bag;
  inst.format = bag.format ?? null;
  inst.minLength = bag.minimum ?? null;
  inst.maxLength = bag.maximum ?? null;
  inst.regex = (...args) => inst.check(_regex(...args));
  inst.includes = (...args) => inst.check(_includes(...args));
  inst.startsWith = (...args) => inst.check(_startsWith(...args));
  inst.endsWith = (...args) => inst.check(_endsWith(...args));
  inst.min = (...args) => inst.check(_minLength(...args));
  inst.max = (...args) => inst.check(_maxLength(...args));
  inst.length = (...args) => inst.check(_length(...args));
  inst.nonempty = (...args) => inst.check(_minLength(1, ...args));
  inst.lowercase = (params) => inst.check(_lowercase(params));
  inst.uppercase = (params) => inst.check(_uppercase(params));
  inst.trim = () => inst.check(_trim());
  inst.normalize = (...args) => inst.check(_normalize(...args));
  inst.toLowerCase = () => inst.check(_toLowerCase());
  inst.toUpperCase = () => inst.check(_toUpperCase());
});
var ZodString = /* @__PURE__ */ $constructor("ZodString", (inst, def) => {
  $ZodString.init(inst, def);
  _ZodString.init(inst, def);
  inst.email = (params) => inst.check(_email(ZodEmail, params));
  inst.url = (params) => inst.check(_url(ZodURL, params));
  inst.jwt = (params) => inst.check(_jwt(ZodJWT, params));
  inst.emoji = (params) => inst.check(_emoji2(ZodEmoji, params));
  inst.guid = (params) => inst.check(_guid(ZodGUID, params));
  inst.uuid = (params) => inst.check(_uuid(ZodUUID, params));
  inst.uuidv4 = (params) => inst.check(_uuidv4(ZodUUID, params));
  inst.uuidv6 = (params) => inst.check(_uuidv6(ZodUUID, params));
  inst.uuidv7 = (params) => inst.check(_uuidv7(ZodUUID, params));
  inst.nanoid = (params) => inst.check(_nanoid(ZodNanoID, params));
  inst.guid = (params) => inst.check(_guid(ZodGUID, params));
  inst.cuid = (params) => inst.check(_cuid(ZodCUID, params));
  inst.cuid2 = (params) => inst.check(_cuid2(ZodCUID2, params));
  inst.ulid = (params) => inst.check(_ulid(ZodULID, params));
  inst.base64 = (params) => inst.check(_base64(ZodBase64, params));
  inst.base64url = (params) => inst.check(_base64url(ZodBase64URL, params));
  inst.xid = (params) => inst.check(_xid(ZodXID, params));
  inst.ksuid = (params) => inst.check(_ksuid(ZodKSUID, params));
  inst.ipv4 = (params) => inst.check(_ipv4(ZodIPv4, params));
  inst.ipv6 = (params) => inst.check(_ipv6(ZodIPv6, params));
  inst.cidrv4 = (params) => inst.check(_cidrv4(ZodCIDRv4, params));
  inst.cidrv6 = (params) => inst.check(_cidrv6(ZodCIDRv6, params));
  inst.e164 = (params) => inst.check(_e164(ZodE164, params));
  inst.datetime = (params) => inst.check(datetime2(params));
  inst.date = (params) => inst.check(date2(params));
  inst.time = (params) => inst.check(time2(params));
  inst.duration = (params) => inst.check(duration2(params));
});
function string2(params) {
  return _string(ZodString, params);
}
var ZodStringFormat = /* @__PURE__ */ $constructor("ZodStringFormat", (inst, def) => {
  $ZodStringFormat.init(inst, def);
  _ZodString.init(inst, def);
});
var ZodEmail = /* @__PURE__ */ $constructor("ZodEmail", (inst, def) => {
  $ZodEmail.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodGUID = /* @__PURE__ */ $constructor("ZodGUID", (inst, def) => {
  $ZodGUID.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodUUID = /* @__PURE__ */ $constructor("ZodUUID", (inst, def) => {
  $ZodUUID.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodURL = /* @__PURE__ */ $constructor("ZodURL", (inst, def) => {
  $ZodURL.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodEmoji = /* @__PURE__ */ $constructor("ZodEmoji", (inst, def) => {
  $ZodEmoji.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodNanoID = /* @__PURE__ */ $constructor("ZodNanoID", (inst, def) => {
  $ZodNanoID.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodCUID = /* @__PURE__ */ $constructor("ZodCUID", (inst, def) => {
  $ZodCUID.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodCUID2 = /* @__PURE__ */ $constructor("ZodCUID2", (inst, def) => {
  $ZodCUID2.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodULID = /* @__PURE__ */ $constructor("ZodULID", (inst, def) => {
  $ZodULID.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodXID = /* @__PURE__ */ $constructor("ZodXID", (inst, def) => {
  $ZodXID.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodKSUID = /* @__PURE__ */ $constructor("ZodKSUID", (inst, def) => {
  $ZodKSUID.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodIPv4 = /* @__PURE__ */ $constructor("ZodIPv4", (inst, def) => {
  $ZodIPv4.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodIPv6 = /* @__PURE__ */ $constructor("ZodIPv6", (inst, def) => {
  $ZodIPv6.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodCIDRv4 = /* @__PURE__ */ $constructor("ZodCIDRv4", (inst, def) => {
  $ZodCIDRv4.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodCIDRv6 = /* @__PURE__ */ $constructor("ZodCIDRv6", (inst, def) => {
  $ZodCIDRv6.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodBase64 = /* @__PURE__ */ $constructor("ZodBase64", (inst, def) => {
  $ZodBase64.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodBase64URL = /* @__PURE__ */ $constructor("ZodBase64URL", (inst, def) => {
  $ZodBase64URL.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodE164 = /* @__PURE__ */ $constructor("ZodE164", (inst, def) => {
  $ZodE164.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodJWT = /* @__PURE__ */ $constructor("ZodJWT", (inst, def) => {
  $ZodJWT.init(inst, def);
  ZodStringFormat.init(inst, def);
});
var ZodNumber = /* @__PURE__ */ $constructor("ZodNumber", (inst, def) => {
  $ZodNumber.init(inst, def);
  ZodType.init(inst, def);
  inst.gt = (value, params) => inst.check(_gt(value, params));
  inst.gte = (value, params) => inst.check(_gte(value, params));
  inst.min = (value, params) => inst.check(_gte(value, params));
  inst.lt = (value, params) => inst.check(_lt(value, params));
  inst.lte = (value, params) => inst.check(_lte(value, params));
  inst.max = (value, params) => inst.check(_lte(value, params));
  inst.int = (params) => inst.check(int(params));
  inst.safe = (params) => inst.check(int(params));
  inst.positive = (params) => inst.check(_gt(0, params));
  inst.nonnegative = (params) => inst.check(_gte(0, params));
  inst.negative = (params) => inst.check(_lt(0, params));
  inst.nonpositive = (params) => inst.check(_lte(0, params));
  inst.multipleOf = (value, params) => inst.check(_multipleOf(value, params));
  inst.step = (value, params) => inst.check(_multipleOf(value, params));
  inst.finite = () => inst;
  const bag = inst._zod.bag;
  inst.minValue = Math.max(bag.minimum ?? Number.NEGATIVE_INFINITY, bag.exclusiveMinimum ?? Number.NEGATIVE_INFINITY) ?? null;
  inst.maxValue = Math.min(bag.maximum ?? Number.POSITIVE_INFINITY, bag.exclusiveMaximum ?? Number.POSITIVE_INFINITY) ?? null;
  inst.isInt = (bag.format ?? "").includes("int") || Number.isSafeInteger(bag.multipleOf ?? 0.5);
  inst.isFinite = true;
  inst.format = bag.format ?? null;
});
function number2(params) {
  return _number(ZodNumber, params);
}
var ZodNumberFormat = /* @__PURE__ */ $constructor("ZodNumberFormat", (inst, def) => {
  $ZodNumberFormat.init(inst, def);
  ZodNumber.init(inst, def);
});
function int(params) {
  return _int(ZodNumberFormat, params);
}
var ZodBoolean = /* @__PURE__ */ $constructor("ZodBoolean", (inst, def) => {
  $ZodBoolean.init(inst, def);
  ZodType.init(inst, def);
});
function boolean2(params) {
  return _boolean(ZodBoolean, params);
}
var ZodUnknown = /* @__PURE__ */ $constructor("ZodUnknown", (inst, def) => {
  $ZodUnknown.init(inst, def);
  ZodType.init(inst, def);
});
function unknown() {
  return _unknown(ZodUnknown);
}
var ZodNever = /* @__PURE__ */ $constructor("ZodNever", (inst, def) => {
  $ZodNever.init(inst, def);
  ZodType.init(inst, def);
});
function never(params) {
  return _never(ZodNever, params);
}
var ZodArray = /* @__PURE__ */ $constructor("ZodArray", (inst, def) => {
  $ZodArray.init(inst, def);
  ZodType.init(inst, def);
  inst.element = def.element;
  inst.min = (minLength, params) => inst.check(_minLength(minLength, params));
  inst.nonempty = (params) => inst.check(_minLength(1, params));
  inst.max = (maxLength, params) => inst.check(_maxLength(maxLength, params));
  inst.length = (len, params) => inst.check(_length(len, params));
  inst.unwrap = () => inst.element;
});
function array(element, params) {
  return _array(ZodArray, element, params);
}
var ZodObject = /* @__PURE__ */ $constructor("ZodObject", (inst, def) => {
  $ZodObjectJIT.init(inst, def);
  ZodType.init(inst, def);
  defineLazy(inst, "shape", () => def.shape);
  inst.keyof = () => _enum(Object.keys(inst._zod.def.shape));
  inst.catchall = (catchall) => inst.clone({ ...inst._zod.def, catchall });
  inst.passthrough = () => inst.clone({ ...inst._zod.def, catchall: unknown() });
  inst.loose = () => inst.clone({ ...inst._zod.def, catchall: unknown() });
  inst.strict = () => inst.clone({ ...inst._zod.def, catchall: never() });
  inst.strip = () => inst.clone({ ...inst._zod.def, catchall: undefined });
  inst.extend = (incoming) => {
    return extend(inst, incoming);
  };
  inst.safeExtend = (incoming) => {
    return safeExtend(inst, incoming);
  };
  inst.merge = (other) => merge(inst, other);
  inst.pick = (mask) => pick(inst, mask);
  inst.omit = (mask) => omit(inst, mask);
  inst.partial = (...args) => partial(ZodOptional, inst, args[0]);
  inst.required = (...args) => required(ZodNonOptional, inst, args[0]);
});
function object(shape, params) {
  const def = {
    type: "object",
    get shape() {
      assignProp(this, "shape", shape ? objectClone(shape) : {});
      return this.shape;
    },
    ...normalizeParams(params)
  };
  return new ZodObject(def);
}
function strictObject(shape, params) {
  return new ZodObject({
    type: "object",
    get shape() {
      assignProp(this, "shape", objectClone(shape));
      return this.shape;
    },
    catchall: never(),
    ...normalizeParams(params)
  });
}
var ZodUnion = /* @__PURE__ */ $constructor("ZodUnion", (inst, def) => {
  $ZodUnion.init(inst, def);
  ZodType.init(inst, def);
  inst.options = def.options;
});
function union(options, params) {
  return new ZodUnion({
    type: "union",
    options,
    ...normalizeParams(params)
  });
}
var ZodDiscriminatedUnion = /* @__PURE__ */ $constructor("ZodDiscriminatedUnion", (inst, def) => {
  ZodUnion.init(inst, def);
  $ZodDiscriminatedUnion.init(inst, def);
});
function discriminatedUnion(discriminator, options, params) {
  return new ZodDiscriminatedUnion({
    type: "union",
    options,
    discriminator,
    ...normalizeParams(params)
  });
}
var ZodIntersection = /* @__PURE__ */ $constructor("ZodIntersection", (inst, def) => {
  $ZodIntersection.init(inst, def);
  ZodType.init(inst, def);
});
function intersection(left, right) {
  return new ZodIntersection({
    type: "intersection",
    left,
    right
  });
}
var ZodRecord = /* @__PURE__ */ $constructor("ZodRecord", (inst, def) => {
  $ZodRecord.init(inst, def);
  ZodType.init(inst, def);
  inst.keyType = def.keyType;
  inst.valueType = def.valueType;
});
function record(keyType, valueType, params) {
  return new ZodRecord({
    type: "record",
    keyType,
    valueType,
    ...normalizeParams(params)
  });
}
var ZodEnum = /* @__PURE__ */ $constructor("ZodEnum", (inst, def) => {
  $ZodEnum.init(inst, def);
  ZodType.init(inst, def);
  inst.enum = def.entries;
  inst.options = Object.values(def.entries);
  const keys = new Set(Object.keys(def.entries));
  inst.extract = (values, params) => {
    const newEntries = {};
    for (const value of values) {
      if (keys.has(value)) {
        newEntries[value] = def.entries[value];
      } else
        throw new Error(`Key ${value} not found in enum`);
    }
    return new ZodEnum({
      ...def,
      checks: [],
      ...normalizeParams(params),
      entries: newEntries
    });
  };
  inst.exclude = (values, params) => {
    const newEntries = { ...def.entries };
    for (const value of values) {
      if (keys.has(value)) {
        delete newEntries[value];
      } else
        throw new Error(`Key ${value} not found in enum`);
    }
    return new ZodEnum({
      ...def,
      checks: [],
      ...normalizeParams(params),
      entries: newEntries
    });
  };
});
function _enum(values, params) {
  const entries = Array.isArray(values) ? Object.fromEntries(values.map((v) => [v, v])) : values;
  return new ZodEnum({
    type: "enum",
    entries,
    ...normalizeParams(params)
  });
}
var ZodLiteral = /* @__PURE__ */ $constructor("ZodLiteral", (inst, def) => {
  $ZodLiteral.init(inst, def);
  ZodType.init(inst, def);
  inst.values = new Set(def.values);
  Object.defineProperty(inst, "value", {
    get() {
      if (def.values.length > 1) {
        throw new Error("This schema contains multiple valid literal values. Use `.values` instead.");
      }
      return def.values[0];
    }
  });
});
function literal(value, params) {
  return new ZodLiteral({
    type: "literal",
    values: Array.isArray(value) ? value : [value],
    ...normalizeParams(params)
  });
}
var ZodTransform = /* @__PURE__ */ $constructor("ZodTransform", (inst, def) => {
  $ZodTransform.init(inst, def);
  ZodType.init(inst, def);
  inst._zod.parse = (payload, _ctx) => {
    if (_ctx.direction === "backward") {
      throw new $ZodEncodeError(inst.constructor.name);
    }
    payload.addIssue = (issue2) => {
      if (typeof issue2 === "string") {
        payload.issues.push(issue(issue2, payload.value, def));
      } else {
        const _issue = issue2;
        if (_issue.fatal)
          _issue.continue = false;
        _issue.code ?? (_issue.code = "custom");
        _issue.input ?? (_issue.input = payload.value);
        _issue.inst ?? (_issue.inst = inst);
        payload.issues.push(issue(_issue));
      }
    };
    const output = def.transform(payload.value, payload);
    if (output instanceof Promise) {
      return output.then((output) => {
        payload.value = output;
        return payload;
      });
    }
    payload.value = output;
    return payload;
  };
});
function transform(fn) {
  return new ZodTransform({
    type: "transform",
    transform: fn
  });
}
var ZodOptional = /* @__PURE__ */ $constructor("ZodOptional", (inst, def) => {
  $ZodOptional.init(inst, def);
  ZodType.init(inst, def);
  inst.unwrap = () => inst._zod.def.innerType;
});
function optional(innerType) {
  return new ZodOptional({
    type: "optional",
    innerType
  });
}
var ZodNullable = /* @__PURE__ */ $constructor("ZodNullable", (inst, def) => {
  $ZodNullable.init(inst, def);
  ZodType.init(inst, def);
  inst.unwrap = () => inst._zod.def.innerType;
});
function nullable(innerType) {
  return new ZodNullable({
    type: "nullable",
    innerType
  });
}
var ZodDefault = /* @__PURE__ */ $constructor("ZodDefault", (inst, def) => {
  $ZodDefault.init(inst, def);
  ZodType.init(inst, def);
  inst.unwrap = () => inst._zod.def.innerType;
  inst.removeDefault = inst.unwrap;
});
function _default(innerType, defaultValue) {
  return new ZodDefault({
    type: "default",
    innerType,
    get defaultValue() {
      return typeof defaultValue === "function" ? defaultValue() : shallowClone(defaultValue);
    }
  });
}
var ZodPrefault = /* @__PURE__ */ $constructor("ZodPrefault", (inst, def) => {
  $ZodPrefault.init(inst, def);
  ZodType.init(inst, def);
  inst.unwrap = () => inst._zod.def.innerType;
});
function prefault(innerType, defaultValue) {
  return new ZodPrefault({
    type: "prefault",
    innerType,
    get defaultValue() {
      return typeof defaultValue === "function" ? defaultValue() : shallowClone(defaultValue);
    }
  });
}
var ZodNonOptional = /* @__PURE__ */ $constructor("ZodNonOptional", (inst, def) => {
  $ZodNonOptional.init(inst, def);
  ZodType.init(inst, def);
  inst.unwrap = () => inst._zod.def.innerType;
});
function nonoptional(innerType, params) {
  return new ZodNonOptional({
    type: "nonoptional",
    innerType,
    ...normalizeParams(params)
  });
}
var ZodCatch = /* @__PURE__ */ $constructor("ZodCatch", (inst, def) => {
  $ZodCatch.init(inst, def);
  ZodType.init(inst, def);
  inst.unwrap = () => inst._zod.def.innerType;
  inst.removeCatch = inst.unwrap;
});
function _catch(innerType, catchValue) {
  return new ZodCatch({
    type: "catch",
    innerType,
    catchValue: typeof catchValue === "function" ? catchValue : () => catchValue
  });
}
var ZodPipe = /* @__PURE__ */ $constructor("ZodPipe", (inst, def) => {
  $ZodPipe.init(inst, def);
  ZodType.init(inst, def);
  inst.in = def.in;
  inst.out = def.out;
});
function pipe(in_, out) {
  return new ZodPipe({
    type: "pipe",
    in: in_,
    out
  });
}
var ZodReadonly = /* @__PURE__ */ $constructor("ZodReadonly", (inst, def) => {
  $ZodReadonly.init(inst, def);
  ZodType.init(inst, def);
  inst.unwrap = () => inst._zod.def.innerType;
});
function readonly(innerType) {
  return new ZodReadonly({
    type: "readonly",
    innerType
  });
}
var ZodCustom = /* @__PURE__ */ $constructor("ZodCustom", (inst, def) => {
  $ZodCustom.init(inst, def);
  ZodType.init(inst, def);
});
function refine(fn, _params = {}) {
  return _refine(ZodCustom, fn, _params);
}
function superRefine(fn) {
  return _superRefine(fn);
}
// packages/toolu-core/src/config/config-schema.ts
var GateModeSchema = _enum(["block", "ask", "advise", "off"]);
var GatePresetSchema = _enum(["strict", "balanced", "relaxed"]);
var DocsSyncModeSchema = _enum(["advise", "block", "off"]);
var AgentTierModeSchema = _enum(["advise", "block", "off"]);
var ModelClassSchema = _enum(["haiku", "sonnet", "opus", "fable", "inherit"]);
var ReasoningEffortSchema = _enum(["low", "medium", "high", "xhigh", "max", "ultra"]);
var GateEntrySchema = object({ mode: GateModeSchema }).strict();
var LangEntrySchema = object({
  maxFileLines: number2().int().positive().optional(),
  maxFnLines: number2().int().positive().optional(),
  maxImplLines: number2().int().positive().optional(),
  noMocks: boolean2().optional()
}).strict();
var CodexModelEntrySchema = object({
  model: string2().min(1),
  reasoningEffort: ReasoningEffortSchema.optional()
}).strict();
var PrBabysitSchema = object({
  dispatch: unknown(),
  hosts: unknown(),
  prefer: unknown(),
  routing: unknown(),
  unattended: unknown(),
  jev: unknown()
}).strict();
var TooluConfigSchema = object({
  version: literal(1).optional(),
  skills: record(string2(), boolean2()).optional(),
  hooks: record(string2(), boolean2()).optional(),
  mcp: record(string2(), boolean2()).optional(),
  agents: record(string2(), boolean2()).optional(),
  models: object({
    enabled: boolean2().optional(),
    mechanical: ModelClassSchema.optional(),
    exploration: ModelClassSchema.optional(),
    implementation: ModelClassSchema.optional(),
    review: ModelClassSchema.optional(),
    synthesis: ModelClassSchema.optional(),
    architecture: ModelClassSchema.optional(),
    codex: record(string2(), CodexModelEntrySchema).optional()
  }).strict().optional(),
  lang: object({
    ts: LangEntrySchema.optional(),
    rust: LangEntrySchema.optional(),
    python: LangEntrySchema.optional()
  }).strict().optional(),
  docsSync: object({
    mode: DocsSyncModeSchema.optional(),
    surfaces: array(string2()).optional(),
    surfaceExcludes: array(string2()).optional(),
    codeSurfaces: array(string2()).optional()
  }).strict().optional(),
  telemetry: object({ enabled: boolean2().optional() }).strict().optional(),
  agentTier: object({ mode: AgentTierModeSchema.optional() }).strict().optional(),
  planLedger: object({ blockOnUncoveredAcs: boolean2().optional() }).strict().optional(),
  gates: object({
    preset: GatePresetSchema.optional(),
    pushReview: GateEntrySchema.optional(),
    qualityGate: GateEntrySchema.optional(),
    commitGate: GateEntrySchema.optional(),
    bashCommands: GateEntrySchema.optional(),
    planLedger: GateEntrySchema.optional(),
    docsSync: GateEntrySchema.optional(),
    agentTier: GateEntrySchema.optional(),
    protectedFiles: GateEntrySchema.optional(),
    mcpBlocker: GateEntrySchema.optional(),
    sweep: boolean2().optional(),
    stateTtlHours: number2().int().positive().optional(),
    telemetryRetentionDays: number2().int().positive().optional()
  }).strict().optional(),
  permissions: object({
    autoAllow: boolean2().optional(),
    allow: array(string2()).optional(),
    deny: array(string2()).optional()
  }).strict().optional(),
  projectSkills: object({
    enabled: boolean2().optional(),
    staleAfterDays: number2().int().positive().optional(),
    archiveAfterDays: number2().int().positive().optional(),
    indexCap: number2().int().positive().optional()
  }).strict().optional(),
  prBabysit: PrBabysitSchema.optional(),
  comemory: record(string2(), unknown()).optional()
}).strict();

// packages/toolu-core/src/config/config-load.ts
var KNOWN_KEYS = new Set(Object.keys(TooluConfigSchema.shape));
function isJsonObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function stderrWarn2(message) {
  process.stderr.write(`toolu-config: ${message}
`);
}
function mergeObjects(user, project) {
  const merged = new Map(Object.entries(user));
  for (const [key, value] of Object.entries(project)) {
    merged.set(key, Object.hasOwn(user, key) ? mergeConfig(user[key], value) : value);
  }
  return Object.fromEntries(merged);
}
function mergeConfig(user, project) {
  return isJsonObject(user) && isJsonObject(project) ? mergeObjects(user, project) : project;
}
function readConfigFile(path) {
  if (!isFile(path)) {
    return { kind: "absent" };
  }
  try {
    const value = JSON.parse(readFileSync2(path, "utf8"));
    return value === null || value === false ? { kind: "malformed" } : { kind: "json", value };
  } catch {
    return { kind: "malformed" };
  }
}
function envelopeError(value) {
  if (!isJsonObject(value)) {
    return "top level is not a JSON object";
  }
  const unknown = Object.keys(value).filter((key) => !KNOWN_KEYS.has(key));
  if (unknown.length > 0) {
    const names = unknown.map((key) => `'${key}'`).join(", ");
    return `unknown top-level key${unknown.length === 1 ? "" : "s"} ${names}`;
  }
  if (value.version !== undefined && value.version !== 1) {
    return `unsupported version ${JSON.stringify(value.version)} (supported: 1)`;
  }
  return;
}
function readLayer(path, warn) {
  const read = path === undefined ? { kind: "absent" } : readConfigFile(path);
  if (read.kind === "absent") {
    return { value: {} };
  }
  if (read.kind === "malformed") {
    warn(`malformed JSON in ${path ?? ""}; ignoring`);
    return { value: {} };
  }
  const error = envelopeError(read.value);
  if (error !== undefined || !isJsonObject(read.value)) {
    const invalid = `${path ?? ""}: ${error ?? "top level is not a JSON object"}`;
    warn(`${invalid}; failing closed (every gate blocks)`);
    return { value: {}, invalid };
  }
  return { value: read.value };
}
function loadConfig(options = {}) {
  const warn = options.warn ?? stderrWarn2;
  const { files, host } = configFiles(options);
  const user = readLayer(files.user, warn);
  const project = readLayer(files.project, warn);
  const invalid = user.invalid ?? project.invalid;
  const data = invalid === undefined ? mergeObjects(user.value, project.value) : {};
  return { data, invalid, files, host, warn };
}

// packages/toolu-core/src/config/config-read.ts
var MODEL_ALIASES = ["haiku", "sonnet", "opus", "fable", "inherit"];
function section(config, key) {
  const value = config.data[key];
  return isJsonObject(value) ? value : undefined;
}
function member(config, category, name) {
  return section(config, category)?.[name];
}
function enabled(config, category, name) {
  const value = member(config, category, name);
  return value !== false && value !== "false";
}

// packages/toolu-core/src/ledger/ledger-parse.ts
var SPACE = "[ \\t\\n\\v\\f\\r]";
var STEPS_HEADING = new RegExp(`^## Steps \\(machine-readable\\)${SPACE}*$`);
var JSON_FENCE = new RegExp(`^\`\`\`json${SPACE}*$`);
var CLOSE_FENCE = new RegExp(`^\`\`\`${SPACE}*$`);
var AC_HEADING = new RegExp(`^## Acceptance criteria${SPACE}*$`);
var AC_ID = /\*\*(AC-[0-9]+):\*\*/;
function isFile2(path) {
  try {
    return statSync3(path).isFile();
  } catch {
    return false;
  }
}
function lines(path) {
  let text;
  try {
    text = readFileSync3(path, "utf8");
  } catch {
    return [];
  }
  const out = text.split(`
`);
  if (out.at(-1) === "")
    out.pop();
  return out;
}
function stepsBlock(path) {
  const captured = [];
  let inSteps = false;
  let inBlock = false;
  for (const line of lines(path)) {
    if (STEPS_HEADING.test(line)) {
      inSteps = true;
    } else if (inSteps && !inBlock && JSON_FENCE.test(line)) {
      inBlock = true;
    } else if (inBlock && CLOSE_FENCE.test(line)) {
      break;
    } else if (inBlock) {
      captured.push(line);
    }
  }
  return captured.join(`
`).replace(/\n+$/, "");
}
function nonEmptyString(value) {
  return typeof value === "string" && value.length > 0;
}
function isStep(value) {
  return isObject(value) && nonEmptyString(value.id) && nonEmptyString(value.title) && nonEmptyString(value.check);
}
function badModels(steps) {
  return steps.filter((step) => (step.model ?? null) !== null).filter((step) => {
    const model = step.model ?? null;
    return typeof model !== "string" || !MODEL_ALIASES.some((alias) => alias === model);
  }).map((step) => `${toStr(step.id ?? null)}=${toStr(step.model ?? null)}`);
}
function badPaths(steps) {
  return steps.filter((step) => (step.paths ?? null) !== null).filter((step) => {
    const paths = step.paths ?? null;
    return !Array.isArray(paths) || paths.some((p) => typeof p !== "string" || p === "");
  }).map((step) => toStr(step.id ?? null));
}
function normalize(step) {
  return {
    ...step,
    ac_refs: alt(step.ac_refs, []),
    depends_on: alt(step.depends_on, []),
    paths: alt(step.paths, []),
    input: alt(step.input, null),
    model: alt(step.model, null)
  };
}
function fail(exitCode, message) {
  return { ok: false, exitCode, message: `plan-ledger-parse: ${message}` };
}
function parseSteps(doc, options = {}) {
  const path = resolve(options.cwd ?? process.cwd(), doc);
  if (!isFile2(path))
    return fail(1, `plan doc not found: ${doc}`);
  const block = stepsBlock(path);
  if (block === "")
    return fail(1, `no '## Steps (machine-readable)' json block in ${doc}`);
  const value = parseJson(block);
  if (!Array.isArray(value) || value.length === 0 || !value.every(isStep)) {
    return fail(1, `steps block in ${doc} is not a non-empty array of {id,title,check} strings`);
  }
  const steps = value;
  const models = badModels(steps);
  if (models.length > 0) {
    return fail(1, `invalid step model in ${doc} (${models.join(", ")}); allowed: ${MODEL_ALIASES.join(" ")}`);
  }
  const paths = badPaths(steps);
  if (paths.length > 0) {
    return fail(2, `invalid step paths in ${doc} (${paths.join(", ")}); expected an array of non-empty strings`);
  }
  return { ok: true, steps: steps.map(normalize) };
}
function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
function docField(doc, field) {
  if (doc === "" || field === "" || !isFile2(doc))
    return "";
  const key = `**${field}:**`;
  const line = lines(doc).find((l) => l.includes(key));
  if (line === undefined)
    return "";
  return line.replace(new RegExp(`^[\\s\\S]*${escapeRegExp(key)}${SPACE}*`), "").replace(new RegExp(`${SPACE}+\\*\\*[^*]+:\\*\\*[\\s\\S]*$`), "").replace(new RegExp(`^${SPACE}+`), "").replace(new RegExp(`${SPACE}+$`), "");
}
function parseAcs(doc) {
  if (doc === "" || !isFile2(doc))
    return [];
  const ids = [];
  let inAc = false;
  for (const line of lines(doc)) {
    if (AC_HEADING.test(line)) {
      inAc = true;
      continue;
    }
    if (inAc && line.startsWith("## "))
      inAc = false;
    const id = inAc ? AC_ID.exec(line)?.[1] : undefined;
    if (id !== undefined && !ids.includes(id))
      ids.push(id);
  }
  return ids;
}
function isSpecless(spec) {
  const lower = spec.replace(/[A-Z]/g, (c) => c.toLowerCase());
  return lower === "" || lower === "none";
}

// packages/toolu-core/src/ledger/ledger-model.ts
var statusIs = (step, status) => jqEquals(get(step, "status"), status);
function isFresh(step, cur, scope, verify) {
  if (!statusIs(step, "green"))
    return false;
  if (!verify) {
    const scoped = alt(get(scope, get(step, "id")), null);
    if (scoped !== null) {
      const own = get(step, "scope_sha");
      return own !== null && jqEquals(own, scoped);
    }
  }
  return jqEquals(get(step, "diff_sha"), cur);
}
function assign(target, key, value) {
  if (target !== null && !isObject(target)) {
    throw new JqError(`Cannot index ${jqType(target)} with "${key}"`);
  }
  return { ...target, [key]: value };
}
function recompute(ledger, cur, scope = {}, verify = false) {
  const steps = each(get(ledger, "steps"));
  const count = (pred) => steps.filter(pred).length;
  const fresh = (step) => isFresh(step, cur, scope, verify);
  const summary = {
    total: length(get(ledger, "steps")),
    green: count((s) => statusIs(s, "green")),
    red: count((s) => statusIs(s, "red")),
    pending: count((s) => statusIs(s, "pending")),
    running: count((s) => statusIs(s, "running")),
    stale: count((s) => statusIs(s, "green") && !fresh(s)),
    fresh_green: count(fresh),
    retried: count((s) => length(alt(get(s, "retries"), [])) > 0)
  };
  const withSummary = assign(ledger, "summary", summary);
  const firstStale = steps.find((s) => !fresh(s));
  const next = firstStale === undefined ? null : alt(get(firstStale, "id"), null);
  return assign(withSummary, "next", next);
}
function summaryLine(ledger, slug) {
  const next = get(ledger, "next");
  const model = each(get(ledger, "steps")).filter((step) => jqEquals(get(step, "id"), next)).map((step) => get(step, "model")).find((m) => m !== null) ?? null;
  const summary = get(ledger, "summary");
  return concat("plan-ledger ", slug, ": ", toStr(get(summary, "fresh_green")), "/", toStr(get(summary, "total")), " fresh-green, next=", alt(next, "none"), model === null ? "" : concat(" model=", model));
}
function allFresh(ledger) {
  return get(ledger, "next") === null;
}
function stepsWithId(steps, id) {
  return steps.filter((step) => jqEquals(get(step, "id"), id));
}
function authored(step) {
  return {
    ac_refs: alt(get(step, "ac_refs"), []),
    depends_on: alt(get(step, "depends_on"), []),
    input: alt(get(step, "input"), null),
    model: alt(get(step, "model"), null)
  };
}
function pendingEntry(step) {
  return {
    id: get(step, "id"),
    title: get(step, "title"),
    check: get(step, "check"),
    status: "pending",
    started_at: null,
    activity: null,
    exit_code: null,
    diff_sha: null,
    last_run: null,
    evidence_tail: null,
    ...authored(step),
    retries: []
  };
}
function runningEntry(step, prior, now, activity) {
  return {
    ...pendingEntry(step),
    status: "running",
    started_at: now,
    activity: activity === "" ? null : activity,
    last_run: now,
    retries: alt(get(prior, "retries"), [])
  };
}
function refreshAuthored(prior, step) {
  let out = prior;
  for (const [key, value] of Object.entries(authored(step)))
    out = assign(out, key, value);
  return out;
}
function carryForward(prior, step) {
  let out = assign(prior, "scope_sha", alt(get(prior, "scope_sha"), null));
  for (const [key, value] of Object.entries(authored(step)))
    out = assign(out, key, value);
  out = assign(out, "title", get(step, "title"));
  out = assign(out, "check", get(step, "check"));
  out = assign(out, "status", alt(get(out, "status"), "pending"));
  for (const key of [
    "started_at",
    "activity",
    "exit_code",
    "diff_sha",
    "last_run",
    "evidence_tail"
  ]) {
    out = assign(out, key, alt(get(out, key), null));
  }
  return assign(out, "retries", alt(get(out, "retries"), []));
}
function appendRetry(retries, record) {
  if (retries === null)
    return [record];
  if (!Array.isArray(retries)) {
    throw new JqError(`${jqType(retries)} and array cannot be added`);
  }
  return [...retries, record];
}
function buildStepEntry(step, prior, run) {
  const priorRetries = alt(get(prior, "retries"), []);
  const retries = jqEquals(alt(get(prior, "status"), null), "red") ? appendRetry(priorRetries, {
    attempt: length(priorRetries) + 1,
    exit_code: get(prior, "exit_code"),
    diff_sha: get(prior, "diff_sha"),
    evidence_tail: get(prior, "evidence_tail"),
    at: get(prior, "last_run")
  }) : priorRetries;
  return {
    id: get(step, "id"),
    title: get(step, "title"),
    check: get(step, "check"),
    status: run.status,
    started_at: null,
    activity: null,
    exit_code: run.exitCode,
    diff_sha: run.sha,
    last_run: run.now,
    evidence_tail: run.evidence,
    ...authored(step),
    retries
  };
}
function beforeCutoff(startedAt, cutoff) {
  if (typeof startedAt === "string")
    return compareJqStrings(startedAt, cutoff) < 0;
  return startedAt === null || typeof startedAt === "boolean" || typeof startedAt === "number";
}
function orphanCutoff(now, thresholdSeconds) {
  return isoSeconds(new Date(Math.floor(now.getTime() / 1000 - thresholdSeconds) * 1000));
}
function healOrphans(ledger, cutoff) {
  const healed = each(get(ledger, "steps")).map((step) => {
    if (!statusIs(step, "running"))
      return step;
    const startedAt = alt(get(step, "started_at"), "");
    if (startedAt !== "" && !beforeCutoff(startedAt, cutoff))
      return step;
    return assign(assign(assign(step, "status", "pending"), "started_at", null), "activity", null);
  });
  return assign(ledger, "steps", healed);
}
function joinIds(ids) {
  return ids.map((id) => {
    if (id === null)
      return "";
    if (typeof id === "object")
      throw new JqError(`Cannot join with ${jqType(id)}`);
    return typeof id === "string" ? id : toJqJson(id, false);
  }).join(", ");
}
function acCoverageLine(ledger, cur, ac) {
  const covering = each(get(ledger, "steps")).filter((step) => holds(alt(get(step, "ac_refs"), []), ac));
  const ids = joinIds(covering.map((step) => get(step, "id")));
  const fresh = covering.some((step) => statusIs(step, "green") && jqEquals(get(step, "diff_sha"), cur));
  if (covering.length === 0)
    return `  ${ac}: UNCOVERED (no step references it)`;
  return fresh ? `  ${ac}: covered by ${ids}` : `  ${ac}: UNCOVERED (${ids} not fresh-green)`;
}
function acCoverage(ledger, cur, spec) {
  if (isSpecless(spec))
    return { stdout: "", stderr: [] };
  const acs = parseAcs(spec);
  if (acs.length === 0)
    return { stdout: "", stderr: [] };
  let stdout = `AC coverage (report-only):
`;
  for (const ac of acs) {
    try {
      stdout += `${acCoverageLine(ledger, cur, ac)}
`;
    } catch (error) {
      if (!(error instanceof JqError))
        throw error;
      return { stdout, stderr: [`plan-ledger: failed to compute AC coverage for ${ac}`] };
    }
  }
  return { stdout, stderr: [] };
}
function entriesById(ledger) {
  const out = new Map;
  for (const step of each(get(ledger, "steps"))) {
    const id = get(step, "id");
    if (typeof id !== "string")
      throw new JqError(`Cannot use ${jqType(id)} as object key`);
    out.set(id, step);
  }
  return out;
}

// packages/toolu-core/src/ledger/ledger-run-context.ts
import { statSync as statSync4 } from "fs";
class CommandFail extends Error {
  lines;
  constructor(lines) {
    super(lines.join(`
`));
    this.lines = lines;
  }
}
function orFail(fn, message) {
  try {
    return fn();
  } catch (error) {
    if (error instanceof JqError)
      throw new CommandFail([message]);
    throw error;
  }
}
function parseRunFlags(flags) {
  const out = { onlyStep: "", activity: "", force: false, verify: false };
  for (let i = 0;i < flags.length; i += 1) {
    const flag = flags[i];
    if (flag === "--step" || flag === "--activity") {
      const value = flags[i + 1] ?? "";
      if (value === "") {
        throw new CommandFail([
          `plan-ledger: ${flag} requires ${flag === "--step" ? "an id" : "a label"}`
        ]);
      }
      if (flag === "--step")
        out.onlyStep = value;
      else
        out.activity = value;
      i += 1;
    } else if (flag === "--force")
      out.force = true;
    else if (flag === "--verify")
      out.verify = true;
    else
      throw new CommandFail([`plan-ledger: unknown run flag: ${flag ?? ""}`]);
  }
  if (out.activity !== "" && out.onlyStep === "") {
    throw new CommandFail(["plan-ledger: --activity requires --step"]);
  }
  return out;
}
function nonEmpty(file) {
  try {
    return statSync4(file).size > 0;
  } catch {
    return false;
  }
}
function need(value, lines) {
  if (value === undefined)
    throw new CommandFail(lines);
  return value;
}
function prepare(doc, flags, options, out) {
  const env = options.env ?? process.env;
  const cwd = options.cwd ?? process.cwd();
  const parsed = parseSteps(doc, { cwd });
  if (!parsed.ok)
    throw new CommandFail([parsed.message]);
  const found = projectRoot2({ env, cwd });
  const base = envValue(env, "PUSH_REVIEW_BASE") ?? (found === undefined ? "main" : baseBranch(found, env));
  const root = need(found, ["plan-ledger: not in a git repo"]);
  const ledgerFile = need(ledgerPath({ ...options, env, cwd }), [
    "plan-ledger: cannot resolve ledger path"
  ]);
  const cur = need(diffSha(root, base, { env }), [`plan-ledger: git diff ${base}...HEAD failed`]);
  const branch = need(headBranch({ env, cwd: root }), ["plan-ledger: cannot resolve HEAD branch"]);
  const prior = readLedger(ledgerFile);
  const corrupt = `plan-ledger: corrupt prior ledger at ${ledgerFile}`;
  if (prior === undefined && nonEmpty(ledgerFile))
    throw new CommandFail([corrupt]);
  const existing = prior === undefined ? new Map : orFail(() => entriesById(prior.value), corrupt);
  const clock = options.now ?? (() => new Date);
  return {
    ...flags,
    doc,
    steps: parsed.steps,
    base,
    root,
    ledgerFile,
    cur,
    branch,
    prior,
    existing,
    scope: {},
    env,
    options,
    out,
    timeout: envValue(env, "PLAN_LEDGER_STEP_TIMEOUT") ?? "1800",
    now: () => isoSeconds(clock())
  };
}
function ledgerDoc(ctx, updatedAt, steps) {
  return {
    version: 1,
    branch: ctx.branch,
    base_branch: ctx.base,
    plan_doc: ctx.doc,
    updated_at: updatedAt,
    summary: {},
    next: null,
    steps
  };
}
function writeOrFail(ctx, ledger, failure) {
  const error = writeLedger(ctx.ledgerFile, ledger);
  if (error !== undefined)
    throw new CommandFail([error, failure]);
}

// packages/toolu-core/src/ledger/ledger-run.ts
import { mkdirSync as mkdirSync4, readFileSync as readFileSync7, rmSync as rmSync3 } from "fs";
import { dirname as dirname4 } from "path";

// packages/toolu-core/src/state/telemetry.ts
import { appendFileSync, mkdirSync as mkdirSync2 } from "fs";
import { join as join4 } from "path";

// packages/toolu-core/src/state/state-schema.ts
var GATE_FILE_VERSION = 1;
var TELEMETRY_VERSION = 1;
var Version = literal(GATE_FILE_VERSION).optional();
var GateEntrySchema2 = strictObject({
  source: string2(),
  reason: string2(),
  violations: string2(),
  updatedAt: string2()
});
var PassingSchema = strictObject({
  version: Version,
  status: literal("passing"),
  source: string2(),
  updatedAt: string2()
});
var FailingSchema = strictObject({
  version: Version,
  status: literal("failing"),
  reason: string2(),
  source: string2(),
  file: string2(),
  violations: string2(),
  entries: record(string2(), GateEntrySchema2).optional(),
  updatedAt: string2()
});
var GateFileSchema = discriminatedUnion("status", [PassingSchema, FailingSchema]);
var text = string2();
var maybeText = string2().nullable();
var TELEMETRY_EXTRAS = {
  gate_fail: strictObject({ file: text, source: text }),
  gate_clear: strictObject({ file: text, source: text }),
  step_run: strictObject({
    step_id: text,
    status: text,
    exit_code: number2(),
    duration_s: number2(),
    attempt: number2()
  }),
  ac_coverage: strictObject({ covered: number2(), uncovered: number2() }),
  docs_attested: strictObject({ decision: text }),
  docs_nudge: strictObject({}),
  push_check: strictObject({ result: text, reason_code: text, round: number2().nullable() }),
  delegation: strictObject({
    model: maybeText,
    subagent_type: maybeText,
    reasoning_effort: maybeText,
    step_id: maybeText,
    step_model: maybeText
  })
};
function isTelemetryEvent(event) {
  return Object.hasOwn(TELEMETRY_EXTRAS, event);
}
var Protocol = {
  v: literal(TELEMETRY_VERSION),
  t: text,
  branch: text
};
var TelemetryLineSchema = discriminatedUnion("event", [
  TELEMETRY_EXTRAS.gate_fail.extend({ ...Protocol, event: literal("gate_fail") }),
  TELEMETRY_EXTRAS.gate_clear.extend({ ...Protocol, event: literal("gate_clear") }),
  TELEMETRY_EXTRAS.step_run.extend({ ...Protocol, event: literal("step_run") }),
  TELEMETRY_EXTRAS.ac_coverage.extend({ ...Protocol, event: literal("ac_coverage") }),
  TELEMETRY_EXTRAS.docs_attested.extend({ ...Protocol, event: literal("docs_attested") }),
  TELEMETRY_EXTRAS.docs_nudge.extend({ ...Protocol, event: literal("docs_nudge") }),
  TELEMETRY_EXTRAS.push_check.extend({ ...Protocol, event: literal("push_check") }),
  TELEMETRY_EXTRAS.delegation.extend({ ...Protocol, event: literal("delegation") })
]);
var EDIT_OPERATIONS = ["add", "update", "delete", "write", "move"];
var EditRecordSchema = strictObject({
  path: string2().min(1),
  operation: _enum(EDIT_OPERATIONS),
  moved_to: string2().optional(),
  from: string2().optional()
});

// packages/toolu-core/src/state/telemetry.ts
var TELEMETRY_MAX_LINE_BYTES = 3900;
function skip(reason) {
  return { written: false, reason };
}
function assemble(event, extras, branch, now) {
  if (!isTelemetryEvent(event)) {
    return new Error(`telemetry: unknown event "${event}"; skipping append`);
  }
  const checked = TELEMETRY_EXTRAS[event].safeParse(extras);
  if (!checked.success) {
    return new Error(`telemetry: invalid extras for event "${event}"; skipping append`);
  }
  const line = toJqJson({ ...checked.data, v: TELEMETRY_VERSION, t: isoSeconds(now), branch, event }, false);
  const bytes = Buffer.byteLength(line, "utf8");
  if (bytes > TELEMETRY_MAX_LINE_BYTES) {
    return new Error(`telemetry: assembled line for event "${event}" is ${String(bytes)} bytes (>${String(TELEMETRY_MAX_LINE_BYTES)}); skipping append`);
  }
  return line;
}
function telemetryAppend(root, event, extras, options = {}) {
  if (root === "")
    return skip("no root");
  const env = options.env ?? process.env;
  const warn = options.warn ?? stderrWarn;
  const host = options.host ?? options.config?.host;
  const scoped = host === undefined ? { env } : { env, host };
  const config = options.config ?? loadConfig({ ...scoped, cwd: root, warn });
  if (!enabled(config, "telemetry", "enabled"))
    return skip("disabled");
  const branch = currentBranch(root, env);
  if (branch === "" || branch === "HEAD")
    return skip("no branch");
  const line = assemble(event, extras, branch, options.now?.() ?? new Date);
  if (line instanceof Error) {
    warn(line.message);
    return skip(line.message);
  }
  const dir = envValue(env, "TELEMETRY_DIR") ?? projectStateDir("telemetry", { ...scoped, host: config.host, root });
  if (dir === undefined)
    return skip("no state dir");
  const file = join4(dir, `${branchSlug(branch)}.jsonl`);
  try {
    mkdirSync2(dir, { recursive: true });
    appendFileSync(file, `${line}
`);
  } catch (error) {
    return skip(`could not append to ${file}: ${String(error)}`);
  }
  return { written: true, file };
}

// packages/toolu-core/src/ledger/ledger-check.ts
import { closeSync, openSync } from "fs";
import { constants as constants2 } from "os";

// packages/toolu-core/src/resources/binding.ts
import { existsSync as existsSync2, readFileSync as readFileSync5, realpathSync, statSync as statSync5 } from "fs";
import { dirname as dirname3, join as join5, resolve as resolve2 } from "path";

// packages/toolu-core/src/resources/lock.ts
import { existsSync, mkdirSync as mkdirSync3, readFileSync as readFileSync4, renameSync as renameSync2, rmSync as rmSync2, writeFileSync as writeFileSync2 } from "fs";
import { dirname as dirname2 } from "path";
import { randomUUID } from "crypto";
function errno(error, code) {
  return error instanceof Error && "code" in error && error.code === code;
}
function processAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0)
    return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (errno(error, "ESRCH"))
      return false;
    throw error;
  }
}
function readJsonFile(path, fallback) {
  try {
    const value = JSON.parse(readFileSync4(path, "utf8"));
    return value;
  } catch (error) {
    if (errno(error, "ENOENT"))
      return fallback;
    throw error;
  }
}
function writeJsonAtomic(path, value) {
  mkdirSync3(dirname2(path), { recursive: true });
  const temp = `${path}.${process.pid}.${randomUUID()}.tmp`;
  try {
    writeFileSync2(temp, `${JSON.stringify(value, null, 2)}
`, { flag: "wx", mode: 384 });
    renameSync2(temp, path);
  } finally {
    rmSync2(temp, { force: true });
  }
}
function stale(path) {
  const owner = readJsonFile(`${path}/owner.json`, null);
  if (isJsonObject(owner) && typeof owner.pid === "number" && typeof owner.token === "string")
    return !processAlive(owner.pid);
  return false;
}
function reclaim(path) {
  const reap = `${path}.reap`;
  try {
    mkdirSync3(reap);
  } catch (error) {
    if (errno(error, "EEXIST"))
      return;
    throw error;
  }
  try {
    if (stale(path))
      rmSync2(path, { recursive: true });
  } finally {
    rmSync2(reap, { recursive: true });
  }
}
async function acquireLock(path, opts = {}) {
  const timeoutMs = opts.timeoutMs ?? 0;
  if (!Number.isFinite(timeoutMs) || timeoutMs < 0 || timeoutMs > 2147483647) {
    throw new Error("lock timeoutMs must be a non-negative finite number within the timer range");
  }
  mkdirSync3(dirname2(path), { recursive: true });
  const until = Date.now() + timeoutMs;
  const attempt = async () => {
    const token = randomUUID();
    const prepared = `${path}.${token}.pending`;
    try {
      if (existsSync(`${path}.reap`))
        return null;
      if (existsSync(path)) {
        reclaim(path);
        if (existsSync(path)) {
          if (Date.now() >= until)
            return null;
          return Bun.sleep(25).then(attempt);
        }
      }
      mkdirSync3(prepared);
      writeJsonAtomic(`${prepared}/owner.json`, { pid: process.pid, token });
      renameSync2(prepared, path);
      return {
        token,
        release: () => {
          const owner = readJsonFile(`${path}/owner.json`, null);
          if (isJsonObject(owner) && owner.token === token) {
            rmSync2(path, { recursive: true });
          }
        }
      };
    } catch (error) {
      if (!errno(error, "EEXIST") && !errno(error, "ENOTEMPTY"))
        throw error;
    } finally {
      rmSync2(prepared, { recursive: true, force: true });
    }
    reclaim(path);
    if (Date.now() >= until)
      return null;
    return Bun.sleep(25).then(attempt);
  };
  return attempt();
}
async function withResourceLock(root, fn) {
  const lock = await acquireLock(`${root}/state.lock`, { timeoutMs: 5000 });
  if (!lock)
    throw new Error("resource state busy; retry later");
  try {
    return await fn();
  } finally {
    lock.release();
  }
}

// packages/toolu-core/src/resources/binding.ts
function gitDir(cwd) {
  let path = realpathSync(cwd);
  for (;; ) {
    const marker = join5(path, ".git");
    if (existsSync2(marker)) {
      if (statSync5(marker).isDirectory())
        return { git: marker, worktree: path };
      const target = /^gitdir: (.+)\s*$/.exec(readFileSync5(marker, "utf8"))?.[1];
      if (!target)
        throw new Error(`invalid git worktree marker: ${marker}`);
      return { git: resolve2(path, target.trim()), worktree: path };
    }
    const parent = dirname3(path);
    if (parent === path)
      return null;
    path = parent;
  }
}
function resourceBinding(cwd) {
  const target = gitDir(cwd);
  if (!target)
    return null;
  const binding = readJsonFile(join5(target.git, "toolu-resource.json"), null);
  if (binding === null)
    return null;
  if (!isJsonObject(binding) || binding.version !== 1 || typeof binding.root !== "string" || !binding.root.startsWith("/") || binding.worktree !== target.worktree || typeof binding.key !== "string" || typeof binding.stateDir !== "string")
    throw new Error("invalid worktree resource binding");
  return {
    version: 1,
    root: binding.root,
    worktree: binding.worktree,
    key: binding.key,
    stateDir: binding.stateDir
  };
}

// packages/toolu-core/src/resources/jobs.ts
import { randomUUID as randomUUID3 } from "crypto";

// packages/toolu-core/src/process/run-command.ts
import { constants } from "os";

// packages/toolu-core/src/process/process-group.ts
var PROCESS_POLL_MS = 25;
var TERMINATE_GRACE_MS = 250;
var KILL_GRACE_MS = 1000;
var PROCESS_TABLE_TIMEOUT_MS = 2000;
var PROCESS_TABLE_MAX_BYTES = 8 * 1024 * 1024;
function errno2(error, code) {
  return error instanceof Error && "code" in error && error.code === code;
}
function validateProcessGroupId(processGroupId) {
  if (!Number.isSafeInteger(processGroupId) || processGroupId <= 0) {
    throw new Error("processGroupId must be a positive safe integer");
  }
}
function groupTarget(processGroupId) {
  return process.platform === "win32" ? processGroupId : -processGroupId;
}
function signalProcessGroup(processGroupId, signal) {
  validateProcessGroupId(processGroupId);
  try {
    process.kill(groupTarget(processGroupId), signal);
  } catch (error) {
    if (!errno2(error, "ESRCH"))
      throw error;
  }
}
function signalProbe(processGroupId) {
  try {
    process.kill(groupTarget(processGroupId), 0);
    return true;
  } catch (error) {
    if (errno2(error, "ESRCH"))
      return false;
    if (errno2(error, "EPERM"))
      return true;
    throw error;
  }
}
function processGroupAlive(processGroupId) {
  validateProcessGroupId(processGroupId);
  if (process.platform === "win32")
    return signalProbe(processGroupId);
  const table = Bun.spawnSync(["ps", "-axo", "pgid=,stat="], {
    stdout: "pipe",
    stderr: "ignore",
    timeout: PROCESS_TABLE_TIMEOUT_MS,
    killSignal: "SIGKILL",
    maxBuffer: PROCESS_TABLE_MAX_BYTES
  });
  if (table.exitCode !== 0)
    return signalProbe(processGroupId);
  const rows = new TextDecoder().decode(table.stdout).split(`
`);
  for (const row of rows) {
    const match = /^\s*(\d+)\s+(\S+)/.exec(row);
    if (match === null || Number(match[1]) !== processGroupId)
      continue;
    if (!match[2]?.startsWith("Z"))
      return true;
  }
  return false;
}
async function waitForDeadGroup(processGroupId, deadline) {
  if (!processGroupAlive(processGroupId))
    return true;
  const remaining = deadline - performance.now();
  if (remaining <= 0)
    return false;
  await Bun.sleep(Math.min(PROCESS_POLL_MS, remaining));
  return waitForDeadGroup(processGroupId, deadline);
}
async function terminateProcessGroup(processGroupId) {
  signalProcessGroup(processGroupId, "SIGTERM");
  if (await waitForDeadGroup(processGroupId, performance.now() + TERMINATE_GRACE_MS))
    return;
  signalProcessGroup(processGroupId, "SIGKILL");
  if (!await waitForDeadGroup(processGroupId, performance.now() + KILL_GRACE_MS)) {
    throw new Error(`process group ${processGroupId} survived SIGKILL`);
  }
}

// packages/toolu-core/src/process/parent-guard.ts
var PARENT_SIGNALS = ["SIGINT", "SIGTERM", "SIGHUP"];
var parentOwnedGroups = new Set;
var parentGuardInstalled = false;
function killParentOwnedGroups() {
  for (const processGroupId of parentOwnedGroups)
    signalProcessGroup(processGroupId, "SIGKILL");
}
function removeParentGuard() {
  if (!parentGuardInstalled)
    return;
  process.off("exit", killParentOwnedGroups);
  for (const signal of PARENT_SIGNALS)
    process.off(signal, forwardParentSignal);
  parentGuardInstalled = false;
}
function forwardParentSignal(signal) {
  killParentOwnedGroups();
  removeParentGuard();
  process.kill(process.pid, signal);
}
function guardParentLifecycle(processGroupId) {
  parentOwnedGroups.add(processGroupId);
  if (!parentGuardInstalled) {
    process.on("exit", killParentOwnedGroups);
    for (const signal of PARENT_SIGNALS)
      process.on(signal, forwardParentSignal);
    parentGuardInstalled = true;
  }
  return () => {
    parentOwnedGroups.delete(processGroupId);
    if (parentOwnedGroups.size === 0)
      removeParentGuard();
  };
}

// packages/toolu-core/src/process/run-command.ts
var DEFAULT_TIMEOUT_MS = 30000;
var DEFAULT_MAX_OUTPUT_BYTES = 1048576;
var MAX_TIMER_MS = 2147483647;
var GROUP_POLL_MS = 250;
var FINAL_DRAIN_MS = 250;
var CANCELLED_EXIT_CODE = 130;
function errno3(error, code) {
  return error instanceof Error && "code" in error && error.code === code;
}
function validateOptions(argv, options) {
  if (argv.length === 0 || argv[0] === undefined || argv[0] === "") {
    throw new Error("argv must not be empty");
  }
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0 || timeoutMs > MAX_TIMER_MS) {
    throw new Error(timeoutMs > MAX_TIMER_MS ? `timeoutMs exceeds the maximum timer delay of ${MAX_TIMER_MS} ms` : "timeoutMs must be a positive finite number");
  }
  const maxOutputBytes = options.maxOutputBytes ?? DEFAULT_MAX_OUTPUT_BYTES;
  if (!Number.isSafeInteger(maxOutputBytes) || maxOutputBytes < 0) {
    throw new Error("maxOutputBytes must be a non-negative safe integer");
  }
  return { timeoutMs, maxOutputBytes };
}
function drain(stream, budget) {
  const reader = stream.getReader();
  const chunks = [];
  const readNext = async () => {
    const next = await reader.read();
    if (next.done)
      return;
    const take = Math.min(next.value.byteLength, budget.remaining);
    if (take > 0)
      chunks.push(next.value.slice(0, take));
    budget.remaining -= take;
    if (take < next.value.byteLength)
      budget.truncated = true;
    return readNext();
  };
  const done = (async () => {
    try {
      await readNext();
    } finally {
      reader.releaseLock();
    }
    const length = chunks.reduce((total, chunk) => total + chunk.byteLength, 0);
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return new TextDecoder().decode(bytes);
  })();
  return {
    done,
    async cancel() {
      try {
        await reader.cancel();
      } catch {}
    }
  };
}
async function feedStdin(proc, input) {
  try {
    await proc.stdin.write(input);
    await proc.stdin.end();
  } catch (error) {
    if (!errno3(error, "EPIPE"))
      throw error;
  }
}
async function waitForGroupExit(processGroupId, stopped) {
  if (!processGroupAlive(processGroupId))
    return;
  const outcome = await Promise.race([
    Bun.sleep(GROUP_POLL_MS).then(() => "poll"),
    stopped
  ]);
  if (outcome === "poll")
    return waitForGroupExit(processGroupId, stopped);
}
async function settleDrains(stdout, stderr) {
  const drained = Promise.all([stdout.done, stderr.done]).then(() => true);
  const finished = await Promise.race([drained, Bun.sleep(FINAL_DRAIN_MS).then(() => false)]);
  if (finished)
    return;
  await Promise.all([stdout.cancel(), stderr.cancel()]);
  await Promise.all([stdout.done, stderr.done]);
}
function exitCode(proc, observed) {
  if (observed !== undefined)
    return observed;
  if (proc.exitCode !== null)
    return proc.exitCode;
  if (typeof proc.signalCode === "number")
    return 128 + proc.signalCode;
  const signalNumber = proc.signalCode === null ? undefined : constants.signals[proc.signalCode];
  return 128 + (signalNumber ?? 0);
}
function stopControl(timeoutMs, signal) {
  const deferred = Promise.withResolvers();
  let current;
  const stop = (reason) => {
    if (current !== undefined)
      return;
    current = reason;
    deferred.resolve(reason);
  };
  const timeout = setTimeout(() => stop("timeout"), timeoutMs);
  const onAbort = () => stop("cancelled");
  signal?.addEventListener("abort", onAbort, { once: true });
  return {
    stopped: deferred.promise,
    reason: () => current,
    release() {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", onAbort);
    }
  };
}
function registerSpawn(callback, pid) {
  try {
    return Promise.resolve(callback?.(pid));
  } catch (error) {
    return Promise.reject(error);
  }
}
async function normalCompletion(proc, prerequisites, stdout, stderr, stop) {
  await Promise.all(prerequisites);
  await waitForGroupExit(proc.pid, stop.stopped);
  if (stop.reason() === undefined)
    await Promise.all([stdout.done, stderr.done]);
}
async function stopCommand(proc, exited, stdout, stderr) {
  await terminateProcessGroup(proc.pid);
  await Promise.allSettled([exited]);
  await settleDrains(stdout, stderr);
}
function commandResult(proc, observedExitCode, started, stop, budget, stdout, stderr) {
  return {
    stdout,
    stderr,
    exitCode: exitCode(proc, observedExitCode),
    durationMs: performance.now() - started,
    timedOut: stop.reason() === "timeout",
    cancelled: stop.reason() === "cancelled",
    truncated: budget.truncated
  };
}
async function monitorCommand(proc, options, started, timeoutMs, maxOutputBytes) {
  const budget = { remaining: maxOutputBytes, truncated: false };
  const stdout = drain(proc.stdout, budget);
  const stderr = drain(proc.stderr, budget);
  const stop = stopControl(timeoutMs, options.signal);
  let observedExitCode;
  const exited = proc.exited.then((code) => observedExitCode = code);
  const registered = registerSpawn(options.onSpawn, proc.pid);
  const fed = registered.then(() => feedStdin(proc, options.stdin ?? ""));
  const normal = normalCompletion(proc, [fed, exited], stdout, stderr, stop);
  const outcome = normal.then(() => ({ kind: "complete" }), (error) => ({ kind: "error", error }));
  try {
    const first = await Promise.race([
      outcome,
      stop.stopped.then(() => ({ kind: "stopped" }))
    ]);
    if (first.kind !== "complete")
      await stopCommand(proc, exited, stdout, stderr);
    if (first.kind === "error")
      throw first.error;
    return commandResult(proc, observedExitCode, started, stop, budget, await stdout.done, await stderr.done);
  } finally {
    stop.release();
  }
}
async function execute(argv, options, timeoutMs, maxOutputBytes) {
  const started = performance.now();
  if (options.signal?.aborted === true) {
    return {
      stdout: "",
      stderr: "",
      exitCode: CANCELLED_EXIT_CODE,
      durationMs: performance.now() - started,
      timedOut: false,
      cancelled: true,
      truncated: false
    };
  }
  const spawnOptions = {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
    detached: true
  };
  if (options.cwd !== undefined)
    spawnOptions.cwd = options.cwd;
  if (options.env !== undefined)
    spawnOptions.env = options.env;
  const proc = Bun.spawn([...argv], spawnOptions);
  const releaseParentGuard = guardParentLifecycle(proc.pid);
  try {
    return await monitorCommand(proc, options, started, timeoutMs, maxOutputBytes);
  } finally {
    releaseParentGuard();
  }
}
function runCommand(argv, options = {}) {
  const { timeoutMs, maxOutputBytes } = validateOptions(argv, options);
  return execute(argv, options, timeoutMs, maxOutputBytes);
}
// packages/toolu-core/src/resources/resources.ts
import { randomUUID as randomUUID2 } from "crypto";

// packages/toolu-core/src/resources/pressure.ts
import { readFileSync as readFileSync6 } from "fs";
import { cpus, freemem, loadavg, totalmem } from "os";
var PRESSURE_SAMPLE_MS = 30000;
var PRESSURE_HOLD_MS = 60000;
var PRESSURE_RECOVERY_MS = 120000;
function finite(value) {
  return Number.isFinite(value);
}
function isSample(sample) {
  if (!isJsonObject(sample))
    return false;
  const ticks = sample.ticks;
  return typeof sample.at === "number" && finite(sample.at) && sample.at >= 0 && typeof sample.cpus === "number" && Number.isSafeInteger(sample.cpus) && sample.cpus > 0 && typeof sample.load === "number" && finite(sample.load) && sample.load >= 0 && typeof sample.availableBytes === "number" && finite(sample.availableBytes) && sample.availableBytes >= 0 && typeof sample.totalBytes === "number" && finite(sample.totalBytes) && sample.totalBytes > 0 && sample.availableBytes <= sample.totalBytes && (sample.steal === null || typeof sample.steal === "number" && finite(sample.steal) && sample.steal >= 0 && sample.steal <= 1) && (ticks === undefined || isJsonObject(ticks) && typeof ticks.total === "number" && Number.isSafeInteger(ticks.total) && ticks.total >= 0 && typeof ticks.steal === "number" && Number.isSafeInteger(ticks.steal) && ticks.steal >= 0);
}
function isPressure(value) {
  if (!isJsonObject(value))
    return false;
  const badSinceValid = value.badSince === null || typeof value.badSince === "number" && finite(value.badSince) && isSample(value.sample) && value.badSince <= value.sample.at;
  const goodSinceValid = value.goodSince === null || typeof value.goodSince === "number" && finite(value.goodSince) && isSample(value.sample) && value.goodSince <= value.sample.at;
  return typeof value.held === "boolean" && badSinceValid && goodSinceValid && isSample(value.sample) && (value.held ? typeof value.reason === "string" : value.reason === null);
}
function advancePressure(previous, sample) {
  if (!isSample(sample) || previous !== undefined && !isPressure(previous)) {
    throw new Error("invalid resource pressure sample");
  }
  if (previous !== undefined && sample.at < previous.sample.at) {
    throw new Error("resource pressure sample moved backwards");
  }
  const memory = sample.availableBytes / sample.totalBytes;
  const effective = Math.max(0.1, sample.cpus * (1 - (sample.steal ?? 0)));
  const bad = sample.load > effective * 1.5 || memory < 0.1 || (sample.steal ?? 0) > 0.25;
  const good = sample.load < effective * 0.8 && memory > 0.2 && (sample.steal ?? 0) < 0.1;
  const badSince = bad ? previous?.badSince ?? sample.at : null;
  const goodSince = good ? previous?.goodSince ?? sample.at : null;
  let held = previous?.held ?? false;
  if (badSince !== null && sample.at - badSince >= PRESSURE_HOLD_MS)
    held = true;
  if (goodSince !== null && sample.at - goodSince >= PRESSURE_RECOVERY_MS)
    held = false;
  return {
    held,
    badSince,
    goodSince,
    sample,
    reason: held ? "sustained CPU/load or memory pressure" : null
  };
}
function sampleResources(previous) {
  const sample = {
    at: Date.now(),
    cpus: cpus().length,
    load: loadavg()[0] ?? 0,
    availableBytes: freemem(),
    totalBytes: totalmem(),
    steal: null
  };
  if (process.platform !== "linux")
    return sample;
  try {
    const cpu = readFileSync6("/proc/stat", "utf8").split(`
`)[0]?.trim().split(/\s+/).slice(1).map(Number) ?? [];
    const ticks = { total: cpu.slice(0, 8).reduce((sum, n) => sum + n, 0), steal: cpu[7] ?? 0 };
    const before = previous?.ticks;
    if (before && ticks.total > before.total)
      sample.steal = Math.max(0, (ticks.steal - before.steal) / (ticks.total - before.total));
    sample.ticks = ticks;
    const available = /^MemAvailable:\s+(\d+) kB$/m.exec(readFileSync6("/proc/meminfo", "utf8"));
    if (available?.[1])
      sample.availableBytes = Number(available[1]) * 1024;
  } catch (error) {
    if (!errno(error, "ENOENT"))
      throw error;
  }
  return sample;
}

// packages/toolu-core/src/resources/resource-store.ts
import { join as join6 } from "path";
function resourcePolicy(root) {
  const p = readJsonFile(join6(root, "policy.json"), {});
  if (!isJsonObject(p))
    throw new Error("invalid resource policy");
  const hosts = {};
  if (p.hosts !== undefined) {
    if (!isJsonObject(p.hosts))
      throw new Error("invalid resource hosts");
    for (const [host, cap] of Object.entries(p.hosts)) {
      if (typeof cap !== "number")
        throw new Error(`invalid resource capacity ${host}`);
      hosts[host] = cap;
    }
  }
  const maxAgents = p.maxAgents ?? 3;
  const maxJobs = p.maxJobs ?? 1;
  if (typeof maxAgents !== "number" || typeof maxJobs !== "number")
    throw new Error("invalid resource capacity");
  const policy = { maxAgents, maxJobs, hosts };
  for (const [key, value] of Object.entries({
    maxAgents: policy.maxAgents,
    maxJobs: policy.maxJobs,
    ...policy.hosts
  })) {
    if (!Number.isSafeInteger(value) || value <= 0)
      throw new Error(`invalid resource capacity ${key}`);
  }
  return policy;
}
function optionalString(value) {
  return value === undefined || typeof value === "string";
}
function isLease(lease) {
  return isJsonObject(lease) && typeof lease.token === "string" && lease.token.length > 0 && (lease.type === "agent" || lease.type === "job") && typeof lease.key === "string" && typeof lease.stateDir === "string" && optionalString(lease.host) && optionalString(lease.pendingHost) && typeof lease.ownerPid === "number" && Number.isSafeInteger(lease.ownerPid) && lease.ownerPid > 0 && (lease.groupPid === undefined || typeof lease.groupPid === "number" && Number.isSafeInteger(lease.groupPid) && lease.groupPid > 0) && optionalString(lease.worktree) && optionalString(lease.pane) && optionalString(lease.session) && typeof lease.stage === "string" && typeof lease.createdAt === "string" && typeof lease.heartbeatAt === "string";
}
function isResourceState(state) {
  return isJsonObject(state) && state.version === 1 && Array.isArray(state.leases) && isJsonObject(state.cooldowns) && state.leases.every(isLease) && Object.values(state.cooldowns).every((value) => isJsonObject(value) && typeof value.until === "number" && Number.isFinite(value.until) && typeof value.reason === "string") && (state.pressure === undefined || isPressure(state.pressure));
}
function readResourceState(root) {
  const state = readJsonFile(join6(root, "state.json"), {
    version: 1,
    leases: [],
    cooldowns: {}
  });
  if (!isResourceState(state))
    throw new Error("invalid resource state; reconcile ownership before launching");
  return state;
}
async function updateResources(root, fn) {
  return withResourceLock(root, () => {
    const state = readResourceState(root);
    try {
      return fn(state);
    } finally {
      writeJsonAtomic(join6(root, "state.json"), state);
    }
  });
}

// packages/toolu-core/src/resources/resources.ts
function reconcileJobs(state) {
  const before = state.leases.length;
  state.leases = state.leases.filter((lease) => lease.type !== "job" || processAlive(lease.ownerPid) || lease.groupPid !== undefined && processGroupAlive(lease.groupPid));
  return before - state.leases.length;
}
function requireJobAdmission(state, req) {
  if (req.type !== "job" || req.worktree === undefined)
    return;
  const agent = state.leases.find((lease) => lease.type === "agent" && lease.worktree === req.worktree);
  if (agent && agent.stage !== "starting" && agent.stage !== "running")
    throw new Error(`worktree job admission blocked by agent stage ${agent.stage}`);
}
function validateRequestCaps(req) {
  if (req.hostCap !== undefined && (!Number.isSafeInteger(req.hostCap) || req.hostCap <= 0))
    throw new Error("invalid host capacity");
  if (req.epicCap !== undefined && (!Number.isSafeInteger(req.epicCap) || req.epicCap <= 0))
    throw new Error("invalid epic capacity");
}
async function acquireLease(root, req) {
  validateRequestCaps(req);
  return updateResources(root, (state) => {
    const policy = resourcePolicy(root);
    reconcileJobs(state);
    if (!state.pressure || Date.now() - state.pressure.sample.at >= PRESSURE_SAMPLE_MS)
      state.pressure = advancePressure(state.pressure, sampleResources(state.pressure?.sample));
    if (state.pressure.held)
      throw new Error(`resource hold: ${state.pressure.reason}`);
    const live = state.leases.filter((lease) => lease.type === req.type);
    requireJobAdmission(state, req);
    if (live.some((lease) => lease.stateDir === req.stateDir && lease.key === req.key))
      throw new Error("existing ownership; reconcile before retry");
    const limit = req.type === "agent" ? policy.maxAgents : policy.maxJobs;
    if (live.length >= limit)
      throw new Error(`${req.type} capacity exhausted (${limit})`);
    if (req.type === "agent" && req.epicCap !== undefined && live.filter((lease) => lease.stateDir === req.stateDir).length >= req.epicCap)
      throw new Error("epic capacity exhausted");
    if (req.type === "agent" && req.host) {
      const hostLimit = Math.min(req.hostCap ?? policy.maxAgents, policy.hosts[req.host] ?? policy.maxAgents);
      if (!Number.isSafeInteger(hostLimit) || hostLimit <= 0)
        throw new Error("invalid host capacity");
      if ((state.cooldowns[req.host]?.until ?? 0) > Date.now())
        throw new Error(`${req.host} is cooling down`);
      if (live.filter((lease) => lease.host === req.host || lease.pendingHost === req.host).length >= hostLimit)
        throw new Error(`${req.host} capacity exhausted`);
    }
    const now = new Date().toISOString();
    const lease = {
      token: randomUUID2(),
      type: req.type,
      key: req.key,
      stateDir: req.stateDir,
      ownerPid: process.pid,
      stage: "starting",
      createdAt: now,
      heartbeatAt: now
    };
    if (req.host !== undefined)
      lease.host = req.host;
    if (req.worktree !== undefined)
      lease.worktree = req.worktree;
    state.leases.push(lease);
    return lease;
  });
}
async function patchLease(root, token, patch) {
  if (patch.groupPid !== undefined && (!Number.isSafeInteger(patch.groupPid) || patch.groupPid <= 0))
    throw new Error("invalid resource lease patch");
  await updateResources(root, (state) => {
    const lease = state.leases.find((candidate) => candidate.token === token);
    if (!lease)
      throw new Error("resource lease lost");
    Object.assign(lease, patch);
  });
}
async function releaseLease(root, token) {
  await updateResources(root, (state) => {
    reconcileJobs(state);
    const lease = state.leases.find((candidate) => candidate.token === token);
    if (lease?.type === "agent" && lease.worktree !== undefined && state.leases.some((candidate) => candidate.type === "job" && candidate.worktree === lease.worktree))
      throw new Error("active jobs prevent agent lease release");
    state.leases = state.leases.filter((candidate) => candidate.token !== token);
  });
}

// packages/toolu-core/src/resources/jobs.ts
var FORWARDED_SIGNALS = ["SIGINT", "SIGTERM", "SIGHUP"];
function noop() {}
function guardOwnedGroup(processGroupId) {
  const onExit = () => signalProcessGroup(processGroupId, "SIGKILL");
  const listeners = FORWARDED_SIGNALS.map((signal) => [
    signal,
    () => {
      signalProcessGroup(processGroupId, "SIGKILL");
      release();
      process.kill(process.pid, signal);
    }
  ]);
  const release = () => {
    process.off("exit", onExit);
    for (const [signal, listener] of listeners)
      process.off(signal, listener);
  };
  process.on("exit", onExit);
  for (const [signal, listener] of listeners)
    process.on(signal, listener);
  return release;
}
async function runManagedJob(argv, binding, opts = {}) {
  const lease = await acquireLease(binding.root, {
    type: "job",
    key: `${binding.key}:${randomUUID3()}`,
    stateDir: binding.stateDir,
    worktree: binding.worktree
  });
  let group;
  let heartbeat;
  let heartbeatFailure;
  let releaseGuard = noop;
  try {
    const input = typeof opts.stdin === "string" ? opts.stdin : new TextDecoder().decode(opts.stdin);
    const result = await runCommand([
      "bash",
      "-c",
      'IFS= read -r ack && [ "$ack" = toolu-go ] && exec "$@"',
      "toolu-job",
      ...argv
    ], {
      ...opts,
      stdin: `toolu-go
${input}`,
      onSpawn: async (pid) => {
        group = pid;
        releaseGuard = guardOwnedGroup(pid);
        await patchLease(binding.root, lease.token, { groupPid: pid, stage: "running" });
        await opts.onSpawn?.(pid);
        heartbeat = setInterval(() => {
          patchLease(binding.root, lease.token, { heartbeatAt: new Date().toISOString() }).catch((error) => {
            heartbeatFailure = error;
          });
        }, 30000);
      }
    });
    if (heartbeatFailure !== undefined)
      throw heartbeatFailure;
    return result;
  } finally {
    releaseGuard();
    clearInterval(heartbeat);
    if (group === undefined || !processGroupAlive(group))
      await releaseLease(binding.root, lease.token);
    else
      await patchLease(binding.root, lease.token, { stage: "cleanup-incomplete" });
  }
}

// packages/toolu-core/src/ledger/ledger-check.ts
var TIMEOUT_EXIT = 124;
var INVALID_TIMEOUT_EXIT = 125;
var TRUNCATED_OUTPUT_NOTE = `plan-ledger: check output exceeded capture limit
`;
var KILL_GRACE_MS2 = 2000;
var EVIDENCE_LINES = 10;
var EVIDENCE_BYTES = 2000;
var UNIT_SECONDS = { "": 1, s: 1, m: 60, h: 3600, d: 86400 };
var MAX_DELAY_MS = 2147483647;
function parseTimeout(value) {
  const match = /^[ \t\n\v\f\r]*\+?((?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)([smhd]?)$/.exec(value);
  if (match === null)
    return;
  const seconds = Number(match[1]) * (UNIT_SECONDS[match[2] ?? ""] ?? 1);
  return Number.isFinite(seconds) ? seconds : undefined;
}
function killGroup(pid, signal) {
  try {
    process.kill(-pid, signal);
  } catch {}
}
var FORWARDED = ["SIGINT", "SIGTERM", "SIGHUP"];
function guardGroup(pid) {
  const onExit = () => killGroup(pid, "SIGKILL");
  const onSignal = (signal) => {
    killGroup(pid, "SIGKILL");
    release();
    process.kill(process.pid, signal);
  };
  const release = () => {
    process.off("exit", onExit);
    for (const signal of FORWARDED)
      process.off(signal, onSignal);
  };
  process.on("exit", onExit);
  for (const signal of FORWARDED)
    process.on(signal, onSignal);
  return release;
}
function exitCodeOf(proc) {
  if (proc.exitCode !== null)
    return proc.exitCode;
  const signal = proc.signalCode;
  const number = signal === null ? undefined : constants2.signals[signal];
  return 128 + (number ?? 0);
}
async function runCheck(run) {
  const seconds = parseTimeout(run.timeout);
  if (seconds === undefined) {
    const note = `plan-ledger: invalid PLAN_LEDGER_STEP_TIMEOUT '${run.timeout}'
`;
    await Bun.write(run.outFile, note);
    return INVALID_TIMEOUT_EXIT;
  }
  const binding = resourceBinding(run.cwd);
  if (binding !== null)
    return runManagedCheck(run, binding, seconds);
  return runUnmanagedCheck(run, seconds);
}
async function runManagedCheck(run, binding, seconds) {
  await Bun.write(run.outFile, "");
  const timeoutMs = seconds === 0 ? MAX_DELAY_MS : Math.min(seconds * 1000, MAX_DELAY_MS);
  const result = await runManagedJob(["bash", "-c", 'exec bash -c "$1" 2>&1', "toolu-ledger", run.check], binding, { cwd: run.cwd, env: run.env, timeoutMs });
  await Bun.write(run.outFile, result.stdout + result.stderr + (result.truncated ? TRUNCATED_OUTPUT_NOTE : ""));
  if (result.timedOut)
    return TIMEOUT_EXIT;
  return result.truncated ? INVALID_TIMEOUT_EXIT : result.exitCode;
}
async function runUnmanagedCheck(run, seconds) {
  const fd = openSync(run.outFile, "w");
  try {
    const proc = Bun.spawn(["bash", "-c", run.check], {
      cwd: run.cwd,
      env: run.env,
      stdin: "ignore",
      stdout: fd,
      stderr: fd,
      detached: true
    });
    const release = guardGroup(proc.pid);
    try {
      return await waitBounded(proc, seconds);
    } finally {
      release();
    }
  } finally {
    closeSync(fd);
  }
}
async function waitBounded(proc, seconds) {
  if (seconds === 0) {
    await proc.exited;
    return exitCodeOf(proc);
  }
  let deadline;
  const expired = new Promise((resolveExpired) => {
    deadline = setTimeout(() => resolveExpired(true), Math.min(seconds * 1000, MAX_DELAY_MS));
  });
  const timedOut = await Promise.race([proc.exited.then(() => false), expired]);
  clearTimeout(deadline);
  if (timedOut) {
    killGroup(proc.pid, "SIGTERM");
    const grace = setTimeout(() => killGroup(proc.pid, "SIGKILL"), KILL_GRACE_MS2);
    await proc.exited;
    clearTimeout(grace);
    killGroup(proc.pid, "SIGKILL");
    return TIMEOUT_EXIT;
  }
  return exitCodeOf(proc);
}
function commandSubstitution(bytes) {
  const kept = bytes.filter((b) => b !== 0);
  let end = kept.length;
  while (end > 0 && kept[end - 1] === 10)
    end -= 1;
  return kept.subarray(0, end);
}
function lastLines(bytes) {
  let seen = 0;
  for (let i = bytes.length - 1;i >= 0; i -= 1) {
    if (bytes[i] === 10) {
      seen += 1;
      if (seen === EVIDENCE_LINES)
        return bytes.subarray(i + 1);
    }
  }
  return bytes;
}
function evidenceOf(bytes) {
  const tail = lastLines(commandSubstitution(bytes)).subarray(0, EVIDENCE_BYTES);
  return new TextDecoder("utf-8").decode(tail);
}
function stepEvidence(exitCode, output, timeout) {
  const evidence = evidenceOf(output);
  if (exitCode !== TIMEOUT_EXIT)
    return evidence;
  const reason = `timed out after ${timeout}s (PLAN_LEDGER_STEP_TIMEOUT)
${toJqJson(evidence, false)}`;
  return evidenceOf(new TextEncoder().encode(reason));
}

// packages/toolu-core/src/ledger/ledger-scope.ts
function scopeSha(base, paths, cwd, env) {
  if (paths.length === 0)
    return;
  const spawnEnv = childEnv(env);
  const diff = Bun.spawnSync(["git", "diff", "--no-color", `${base}...HEAD`, "--", ...paths], {
    cwd,
    env: spawnEnv,
    stdout: "pipe",
    stderr: "ignore"
  });
  if (!diff.success)
    return;
  const declared = new TextEncoder().encode(paths.map((p) => `${p}\x00`).join(""));
  const body = commandSubstitution(diff.stdout);
  const input = new Uint8Array(declared.length + body.length);
  input.set(declared, 0);
  input.set(body, declared.length);
  const hash = Bun.spawnSync(["git", "hash-object", "--stdin"], {
    cwd,
    env: spawnEnv,
    stdin: input,
    stdout: "pipe",
    stderr: "ignore"
  });
  const sha = hash.success ? hash.stdout.toString("utf8").trim() : "";
  return sha === "" ? undefined : sha;
}
function scopeMap(steps, base, cwd, env, warn) {
  const out = {};
  for (const step of steps) {
    const paths = get(step, "paths");
    if (!Array.isArray(paths) || paths.length === 0)
      continue;
    const id = raw(get(step, "id"));
    if (id === "")
      continue;
    const sha = scopeSha(base, eachOptional(paths).map(raw), cwd, env);
    if (sha === undefined) {
      warn(`plan-ledger: step ${id} declares paths that could not be hashed; judging it on the whole branch diff`);
      continue;
    }
    out[id] = sha;
  }
  return out;
}

// packages/toolu-core/src/ledger/ledger-run.ts
function preWrite(ctx) {
  const startedAt = ctx.now();
  const steps = orFail(() => ctx.steps.flatMap((step) => {
    const prior = ctx.existing.get(step.id) ?? null;
    return stepsWithId(ctx.steps, step.id).map((match) => {
      if (step.id === ctx.onlyStep)
        return runningEntry(match, prior, startedAt, ctx.activity);
      return prior === null ? pendingEntry(match) : refreshAuthored(prior, match);
    });
  }), "plan-ledger: failed to assemble running pre-write");
  const ledger = orFail(() => recompute(ledgerDoc(ctx, startedAt, steps), ctx.cur, {}, ctx.verify), "plan-ledger: failed to recompute running pre-ledger");
  writeOrFail(ctx, ledger, "plan-ledger: running pre-write failed");
}
function progress(ctx, message) {
  ctx.out.stderr(`plan-ledger: ${message}`);
}
async function runStep(ctx, id, matches, prior, at, scopeNow) {
  const tmp = `${ctx.ledgerFile}.run.${process.pid}.${id}`;
  try {
    mkdirSync4(dirname4(ctx.ledgerFile), { recursive: true });
  } catch {}
  progress(ctx, `${at} ${id}: running check`);
  const check = matches.map((m) => raw(get(m, "check"))).join(`
`).replace(/\n+$/, "");
  const t0 = Math.floor(Date.now() / 1000);
  let code = 1;
  let output = new Uint8Array;
  try {
    code = await runCheck({
      check,
      cwd: ctx.root,
      env: childEnv(ctx.env),
      outFile: tmp,
      timeout: ctx.timeout
    });
    output = readFileSync7(tmp);
  } catch {} finally {
    rmSync3(tmp, { force: true });
  }
  const duration = Math.floor(Date.now() / 1000) - t0;
  const status = code === 0 ? "green" : "red";
  const evidence = stepEvidence(code, output, ctx.timeout);
  progress(ctx, `${at} ${id}: ${status} (${String(duration)}s)`);
  const run = { status, exitCode: code, sha: ctx.cur, evidence, now: ctx.now() };
  const entries = orFail(() => matches.map((m) => ({
    ...buildStepEntry(m, prior, run),
    scope_sha: scopeNow === "" ? null : scopeNow
  })), `plan-ledger: failed to build entry for step ${id}`);
  const [entry] = entries;
  if (entries.length === 1 && entry !== undefined) {
    const retries = entry.retries;
    const attempt = (Array.isArray(retries) ? retries.length : 0) + 1;
    const extras = { step_id: id, status, exit_code: code, duration_s: duration, attempt };
    const warn = (line) => ctx.out.stderr(line);
    const hostOpt = ctx.options.host === undefined ? {} : { host: ctx.options.host };
    const nowOpt = ctx.options.now === undefined ? {} : { now: ctx.options.now };
    telemetryAppend(ctx.root, "step_run", extras, { env: ctx.env, warn, ...hostOpt, ...nowOpt });
  }
  return entries;
}
async function runOrSkip(ctx, id, matches, at) {
  const prior = ctx.existing.get(id) ?? null;
  const scoped = ctx.scope[id];
  const scopeNow = typeof scoped === "string" ? scoped : "";
  const useScope = !ctx.verify && scopeNow !== "";
  const priorKey = raw(alt(get(prior, useScope ? "scope_sha" : "diff_sha"), ""));
  const nowKey = useScope ? scopeNow : ctx.cur;
  const green = raw(alt(get(prior, "status"), "")) === "green";
  if (!ctx.force && ctx.onlyStep === "" && green && nowKey !== "" && priorKey === nowKey) {
    progress(ctx, `${at} ${id}: fresh-green, skipped (--force re-runs)`);
    return orFail(() => matches.map((m) => carryForward(prior, m)), `plan-ledger: failed to carry forward step ${id}`);
  }
  return runStep(ctx, id, matches, prior, at, scopeNow);
}
async function runSteps(ctx) {
  const out = [];
  let index = 0;
  await ctx.steps.reduce(async (previous, { id }) => {
    await previous;
    const matches = stepsWithId(ctx.steps, id);
    let entries;
    if (ctx.onlyStep !== "" && id !== ctx.onlyStep) {
      const prior = ctx.existing.get(id) ?? null;
      entries = orFail(() => matches.map((m) => prior === null ? pendingEntry(m) : carryForward(prior, m)), `plan-ledger: failed to assemble entry for step ${id}`);
    } else {
      index += 1;
      entries = await runOrSkip(ctx, id, matches, `[${String(index)}/${String(ctx.steps.length)}]`);
    }
    const [entry] = entries;
    if (entries.length !== 1 || entry === undefined) {
      throw new CommandFail([`plan-ledger: failed to append step ${id}`]);
    }
    out.push(entry);
  }, Promise.resolve());
  return out;
}
function priorVerified(prior) {
  try {
    return prior === undefined ? "" : raw(alt(get(prior.value, "verified_sha"), ""));
  } catch (error) {
    if (error instanceof JqError)
      return "";
    throw error;
  }
}
function finish(ctx, steps) {
  const ledger = orFail(() => recompute(ledgerDoc(ctx, ctx.now(), steps), ctx.cur, ctx.scope, ctx.verify), "plan-ledger: failed to recompute summary");
  if (!isObject(ledger))
    throw new CommandFail(["plan-ledger: failed to recompute summary"]);
  const summary = get(ledger, "summary");
  const verified = ctx.verify && ctx.onlyStep === "" && jqEquals(get(summary, "fresh_green"), get(summary, "total"));
  const previous = priorVerified(ctx.prior);
  const stamped = {
    ...ledger,
    verified_sha: verified ? ctx.cur : previous === "" ? null : previous
  };
  writeOrFail(ctx, stamped, "plan-ledger: ledger write failed");
  try {
    ctx.out.stdout(`${summaryLine(stamped, branchSlug(ctx.branch))}
`);
  } catch (error) {
    if (!(error instanceof JqError))
      throw error;
  }
  return stamped;
}
async function ledgerRun(doc, flags, options = {}) {
  const out = new Output(options.onStderr);
  try {
    const ctx = prepare(doc, parseRunFlags(flags), options, out);
    if (ctx.onlyStep !== "")
      preWrite(ctx);
    ctx.scope = scopeMap(ctx.steps, ctx.base, ctx.root, ctx.env, (line) => out.stderr(line));
    const ledger = finish(ctx, await runSteps(ctx));
    return out.result(allFresh(ledger) ? 0 : 1);
  } catch (error) {
    if (!(error instanceof CommandFail))
      throw error;
    for (const line of error.lines)
      out.stderr(line);
    return out.result(2);
  }
}

// packages/toolu-core/src/ledger/ledger-commands.ts
var DEFAULT_STUCK_SECONDS = 300;
function stuckThreshold(options) {
  const value = envValue(options.env ?? process.env, "PL_STUCK_THRESHOLD");
  return value !== undefined && /^-?\d+$/.test(value) ? Number(value) : DEFAULT_STUCK_SECONDS;
}
function cutoff(options) {
  return orphanCutoff(options.now?.() ?? new Date, stuckThreshold(options));
}
function baseFor(options, root) {
  const env = options.env ?? process.env;
  return envValue(env, "PUSH_REVIEW_BASE") ?? (root === undefined ? "main" : baseBranch(root, env));
}
function fileOrUnderRoot(path, cwd, root) {
  if (isFile2(resolve3(cwd, path)))
    return resolve3(cwd, path);
  if (root !== undefined && isFile2(join7(root, path)))
    return join7(root, path);
  return;
}
function coverageReport(ledger, cur, options, out) {
  const cwd = options.cwd ?? process.cwd();
  const planDoc = raw(alt(get(ledger, "plan_doc"), ""));
  if (planDoc === "")
    return;
  const root = projectRoot2(options);
  const plan = fileOrUnderRoot(planDoc, cwd, root);
  if (plan === undefined)
    return;
  const specField = docField(plan, "Spec");
  const spec = isSpecless(specField) ? specField : fileOrUnderRoot(specField, cwd, root) ?? resolve3(cwd, specField);
  const report = acCoverage(ledger, cur, spec);
  out.stdout(report.stdout);
  for (const line of report.stderr)
    out.stderr(line);
}
function ledgerStatus(options = {}) {
  const out = new Output(options.onStderr);
  try {
    const env = options.env ?? process.env;
    const base = baseFor(options, projectRoot2(options));
    const file = ledgerPath(options);
    if (file === undefined)
      throw new CommandFail(["plan-ledger: cannot resolve ledger path"]);
    const read = readLedger(file);
    if (read === undefined)
      throw new CommandFail([`plan-ledger: no ledger at ${file}`]);
    const cur = diffSha(options.cwd ?? process.cwd(), base, { env });
    if (cur === undefined)
      throw new CommandFail([`plan-ledger: git diff ${base}...HEAD failed`]);
    const branch = headBranch(options);
    if (branch === undefined)
      throw new CommandFail(["plan-ledger: cannot resolve HEAD branch"]);
    const healed = orFail(() => healOrphans(read.value, cutoff(options)), "plan-ledger: failed to heal orphaned running steps");
    const ledger = orFail(() => recompute(healed, cur), "plan-ledger: failed to recompute summary");
    const error = writeLedger(file, ledger);
    if (error !== undefined)
      throw new CommandFail([error, "plan-ledger: ledger write failed"]);
    try {
      out.stdout(`${summaryLine(ledger, branchSlug(branch))}
`);
    } catch (failure) {
      if (!(failure instanceof JqError))
        throw failure;
    }
    coverageReport(ledger, cur, options, out);
    return out.result(allFresh(ledger) ? 0 : 1);
  } catch (error) {
    if (!(error instanceof CommandFail))
      throw error;
    for (const line of error.lines)
      out.stderr(line);
    return out.result(2);
  }
}
function readable(path) {
  try {
    accessSync(path, constants3.R_OK);
    return true;
  } catch {
    return false;
  }
}
var isApproved = (status) => status.replace(/[A-Z]/g, (c) => c.toLowerCase()) === "approved";
function healLedgerFor(options) {
  const file = ledgerPath(options);
  const read = file === undefined ? undefined : readLedger(file);
  if (file === undefined || read === undefined)
    return "";
  try {
    const healed = healOrphans(read.value, cutoff(options));
    if (toJqJson(healed, true) !== read.text)
      writeLedger(file, healed);
  } catch (error) {
    if (!(error instanceof JqError))
      throw error;
  }
  try {
    return raw(alt(get(read.value, "plan_doc"), ""));
  } catch (error) {
    if (!(error instanceof JqError))
      throw error;
    return "";
  }
}
function reviewRemedy(host, phase) {
  const action = host === "opencode" ? 'load skill({ name: "delivery-flow-delivery-flow" })' : host === "claude" || host === "codex" ? "run /delivery-flow:delivery-flow" : "load the delivery-flow skill";
  return `${action} (${phase} review phase)`;
}
function preflightChecks(plan, root, cwd, host) {
  const under = (path) => isAbsolute(path) || root === undefined ? resolve3(cwd, path) : join7(root, path);
  const planAbs = under(plan);
  if (!readable(planAbs))
    return { code: 2, line: `preflight: plan doc not found or unreadable: ${plan}` };
  const status = docField(planAbs, "Status");
  const planReview = reviewRemedy(host, "plan");
  if (status === "")
    return {
      code: 1,
      line: `preflight: plan has no **Status:** header (${plan}) \u2014 ${planReview} to stamp it`
    };
  if (!isApproved(status))
    return { code: 1, line: `preflight: plan not approved (Status: ${status}) \u2014 ${planReview}` };
  const spec = docField(planAbs, "Spec");
  if (isSpecless(spec))
    return { code: 0 };
  const specAbs = under(spec);
  if (!readable(specAbs))
    return { code: 1, line: `preflight: declared spec not found or unreadable: ${spec}` };
  const specStatus = docField(specAbs, "Status");
  if (isApproved(specStatus))
    return { code: 0 };
  const shown = specStatus === "" ? "none" : specStatus;
  return {
    code: 1,
    line: `preflight: spec ${spec} not approved (Status: ${shown}) \u2014 ${reviewRemedy(host, "spec")}`
  };
}
function ledgerPreflight(plan = "", options = {}) {
  const out = new Output(options.onStderr);
  const root = projectRoot2(options);
  const fromLedger = healLedgerFor(options);
  const target = plan === "" ? fromLedger : plan;
  if (target === "") {
    out.stderr("preflight: no plan doc given and no ledger plan_doc to resolve");
    return out.result(2);
  }
  const { host } = resolveHost(options);
  const verdict = preflightChecks(target, root, options.cwd ?? process.cwd(), host);
  if (verdict.line !== undefined)
    out.stderr(verdict.line);
  return out.result(verdict.code);
}
var SELF_TEST_DOC = `# Self-test Plan

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "ok", "check": "true" },
  { "id": "s2", "title": "fail", "check": "false" }
]
\`\`\`
`;
function ledgerSelfTest(options = {}) {
  const out = new Output(options.onStderr);
  const dir = mkdtempSync(join7(tmpdir(), "plan-ledger-self-test-"));
  const doc = join7(dir, "selftest-plan.md");
  writeFileSync3(doc, SELF_TEST_DOC);
  const parsed = parseSteps(doc);
  rmSync4(dir, { recursive: true, force: true });
  if (!parsed.ok) {
    out.stderr(parsed.message);
    out.stderr("plan-ledger --self-test: parse failed");
    return out.result(1);
  }
  const [first, second] = parsed.steps;
  if (parsed.steps.length !== 2 || first?.id !== "s1" || second?.check !== "false") {
    out.stderr("plan-ledger --self-test: unexpected parse result");
    return out.result(1);
  }
  out.stdout(`plan-ledger --self-test: ok
`);
  return out.result(0);
}
var USAGE = "plan-ledger: usage: run <doc> [--step <id>] [--activity <label>] | status | preflight [<doc>] | path | root | --self-test";
function fail2(line, options) {
  const out = new Output(options.onStderr);
  out.stderr(line);
  return out.result(2);
}
function printed(value, options) {
  const out = new Output(options.onStderr);
  out.stdout(`${value}
`);
  return out.result(0);
}
async function ledgerMain(argv, options = {}) {
  if (!hasGit(options.env ?? process.env))
    return fail2("plan-ledger: git is required", options);
  const [command = "", ...rest] = argv;
  switch (command) {
    case "run": {
      const [doc = "", ...flags] = rest;
      if (doc === "")
        return fail2("plan-ledger: run requires a plan doc path", options);
      return ledgerRun(doc, flags, options);
    }
    case "status":
      return ledgerStatus(options);
    case "preflight":
      return ledgerPreflight(rest[0] ?? "", options);
    case "path": {
      const path = ledgerPath(options);
      return path === undefined ? fail2("plan-ledger: cannot resolve ledger path", options) : printed(path, options);
    }
    case "root": {
      const root = projectRoot2(options);
      return root === undefined ? fail2("plan-ledger: not in a git repo", options) : printed(root, options);
    }
    case "--self-test":
      return ledgerSelfTest(options);
    default:
      return fail2(USAGE, options);
  }
}
// plugins/toolu/hooks/src/plan-ledger.ts
var result = await ledgerMain(process.argv.slice(2));
process.stdout.write(result.stdout);
process.stderr.write(result.stderr);
process.exitCode = result.exitCode;
