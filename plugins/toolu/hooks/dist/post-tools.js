// @bun
// plugins/toolu/hooks/src/post-tools.ts
import { join as join9 } from "path";

// packages/toolu-core/src/config/config-load.ts
import { readFileSync } from "fs";

// packages/toolu-core/src/config/config-files.ts
import { statSync } from "fs";
import { join as join2 } from "path";

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
function nativeEventName(host, event) {
  return TABLES[host][event];
}
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
import { spawnSync } from "child_process";
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

// packages/toolu-core/src/config/config-files.ts
function isFile(path) {
  try {
    return statSync(path).isFile();
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
    user: join2(userDir, "toolu.config.json"),
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
function isObject(data) {
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
  if (isObject(o) === false)
    return false;
  const ctor = o.constructor;
  if (ctor === undefined)
    return true;
  const prot = ctor.prototype;
  if (isObject(prot) === false)
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
  const isObject2 = isObject;
  const catchall = def.catchall;
  let value;
  inst._zod.parse = (payload, ctx) => {
    value ?? (value = _normalized.value);
    const input = payload.value;
    if (!isObject2(input)) {
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
  const isObject2 = isObject;
  const jit = !globalConfig.jitless;
  const allowsEval2 = allowsEval;
  const fastEnabled = jit && allowsEval2.value;
  const catchall = def.catchall;
  let value;
  inst._zod.parse = (payload, ctx) => {
    value ?? (value = _normalized.value);
    const input = payload.value;
    if (!isObject2(input)) {
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
    if (!isObject(input)) {
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
function _custom(Class, fn, _params) {
  const norm = normalizeParams(_params);
  norm.abort ?? (norm.abort = true);
  const schema = new Class({
    type: "custom",
    check: "custom",
    fn,
    ...norm
  });
  return schema;
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
function looseObject(shape, params) {
  return new ZodObject({
    type: "object",
    get shape() {
      assignProp(this, "shape", objectClone(shape));
      return this.shape;
    },
    catchall: unknown(),
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
function custom(fn, _params) {
  return _custom(ZodCustom, fn ?? (() => true), _params);
}
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
function stderrWarn(message) {
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
    const value = JSON.parse(readFileSync(path, "utf8"));
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
  const warn = options.warn ?? stderrWarn;
  const { files, host } = configFiles(options);
  const user = readLayer(files.user, warn);
  const project = readLayer(files.project, warn);
  const invalid = user.invalid ?? project.invalid;
  const data = invalid === undefined ? mergeObjects(user.value, project.value) : {};
  return { data, invalid, files, host, warn };
}

// packages/toolu-core/src/config/config-read.ts
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

// packages/toolu-core/src/state/state-io.ts
import { randomUUID } from "crypto";
import { linkSync, readFileSync as readFileSync2, renameSync, rmSync, statSync as statSync2, writeFileSync } from "fs";
var stderrWarn2 = (message) => {
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
function isErrno(error, code) {
  return error instanceof Error && "code" in error && error.code === code;
}
function writeAtomic(file, body) {
  const tmp = `${file}.${randomUUID()}.tmp`;
  try {
    writeFileSync(tmp, body, { flag: "wx", mode: 384 });
    renameSync(tmp, file);
    return true;
  } catch {
    try {
      rmSync(tmp, { force: true });
    } catch {}
    return false;
  }
}
var LOCK_POLL_MS = 10;
var DEFAULT_LOCK_TIMEOUT_MS = 5000;
var DEFAULT_LOCK_STALE_MS = 2000;
function lockContent(lock) {
  try {
    return readFileSync2(lock, "utf8");
  } catch {
    return;
  }
}
function holderDead(content) {
  const pid = Number(content.split(" ")[0]);
  if (!Number.isInteger(pid) || pid <= 0)
    return false;
  try {
    process.kill(pid, 0);
    return false;
  } catch (error) {
    return isErrno(error, "ESRCH");
  }
}
function breakIfStale(lock, staleMs) {
  const content = lockContent(lock);
  const stat = statSync2(lock, { throwIfNoEntry: false });
  if (content === undefined || stat === undefined)
    return;
  if (!holderDead(content) && Date.now() - stat.mtimeMs <= staleMs)
    return;
  const claimed = `${lock}.${randomUUID()}.broken`;
  try {
    renameSync(lock, claimed);
  } catch {
    return;
  }
  if (lockContent(claimed) !== content) {
    try {
      linkSync(claimed, lock);
    } catch {}
  }
  rmSync(claimed, { force: true });
}
function tryLock(lock, content) {
  try {
    writeFileSync(lock, content, { flag: "wx", mode: 384 });
    return "held";
  } catch (error) {
    return isErrno(error, "EEXIST") ? "busy" : "unavailable";
  }
}
function withLock(file, fn, options = {}) {
  const lock = `${file}.lock`;
  const ours = `${String(process.pid)} ${randomUUID()}
`;
  const deadline = Date.now() + (options.timeoutMs ?? DEFAULT_LOCK_TIMEOUT_MS);
  let state = tryLock(lock, ours);
  while (state === "busy") {
    if (Date.now() >= deadline) {
      (options.warn ?? stderrWarn2)(`state: lock ${lock} still held; writing without it`);
      break;
    }
    breakIfStale(lock, options.staleMs ?? DEFAULT_LOCK_STALE_MS);
    state = tryLock(lock, ours);
    if (state === "busy")
      Bun.sleepSync(LOCK_POLL_MS);
  }
  try {
    return fn();
  } finally {
    if (state === "held" && lockContent(lock) === ours)
      rmSync(lock, { force: true });
  }
}

// packages/toolu-core/src/state/edit-records.ts
var EDIT_TOOLS = ["Edit", "Write", "MultiEdit", "apply_patch"];
function isEditTool(tool) {
  return EDIT_TOOLS.some((name) => name === tool);
}
function pathValid(path) {
  return path !== "" && !/[\n\r\t]/.test(path);
}
function substituted(value) {
  return value.replace(/\n+$/, "");
}
function toolInput(payload) {
  if (payload === null)
    return null;
  if (!isJsonObject(payload))
    return;
  const input = payload.tool_input;
  if (input === undefined || input === null)
    return null;
  return isJsonObject(input) ? input : undefined;
}
function truthy(value) {
  return value !== undefined && value !== null && value !== false;
}
function editPath(input) {
  const value = [input?.file_path, input?.path].find(truthy) ?? input?.target_file;
  return typeof value === "string" ? substituted(value) : undefined;
}
function flushPending(s) {
  if (s.pending !== "")
    s.records.push({ path: s.pending, operation: "update" });
  s.pending = "";
}
function headerPath(line, prefix) {
  const raw = line.slice(prefix.length);
  if (!raw.startsWith(" "))
    return;
  const path = raw.slice(1);
  return pathValid(path) ? path : undefined;
}
function fileHeader(s, line, prefix, operation) {
  if (!s.begun) {
    s.invalid = true;
    return;
  }
  flushPending(s);
  const path = headerPath(line, prefix);
  if (path === undefined) {
    s.invalid = true;
    return;
  }
  s.records.push({ path, operation });
  s.headers += 1;
}
function updateHeader(s, line) {
  if (!s.begun) {
    s.invalid = true;
    return;
  }
  flushPending(s);
  const path = headerPath(line, "*** Update File:");
  if (path === undefined) {
    s.invalid = true;
    return;
  }
  s.pending = path;
  s.headers += 1;
}
function moveHeader(s, line) {
  const target = s.begun && s.pending !== "" ? headerPath(line, "*** Move to:") : undefined;
  if (target === undefined) {
    s.invalid = true;
    return;
  }
  s.records.push({ path: s.pending, operation: "update", moved_to: target });
  s.records.push({ path: target, operation: "move", from: s.pending });
  s.pending = "";
  s.headers += 1;
}
function patchLine(s, line) {
  if (s.ended) {
    if (line !== "")
      s.invalid = true;
  } else if (line === "*** Begin Patch") {
    if (s.begun)
      s.invalid = true;
    s.begun = true;
  } else if (line === "*** End Patch") {
    if (s.begun) {
      flushPending(s);
      s.ended = true;
    } else {
      s.invalid = true;
    }
  } else if (line.startsWith("*** Add File:")) {
    fileHeader(s, line, "*** Add File:", "add");
  } else if (line.startsWith("*** Update File:")) {
    updateHeader(s, line);
  } else if (line.startsWith("*** Delete File:")) {
    fileHeader(s, line, "*** Delete File:", "delete");
  } else if (line.startsWith("*** Move to:")) {
    moveHeader(s, line);
  } else if (line === "*** End of File") {
    if (!s.begun || s.headers === 0)
      s.invalid = true;
  } else if (line.startsWith("*** ")) {
    s.invalid = true;
  }
}
function applyPatchRecords(patch) {
  const s = {
    records: [],
    pending: "",
    begun: false,
    ended: false,
    headers: 0,
    invalid: false
  };
  for (const line of patch.split(`
`)) {
    patchLine(s, line.replace(/\r$/, ""));
  }
  return s.invalid || !s.begun || !s.ended || s.headers === 0 ? undefined : s.records;
}
function normalizeEditRecords(payload, tool) {
  if (!isEditTool(tool))
    return { kind: "not-edit" };
  const input = toolInput(payload);
  if (input === undefined)
    return { kind: "malformed" };
  if (tool === "apply_patch") {
    const command = input?.command;
    const records = typeof command === "string" ? applyPatchRecords(substituted(command)) : undefined;
    return records === undefined ? { kind: "malformed" } : { kind: "records", records };
  }
  const path = editPath(input);
  if (path === undefined || !pathValid(path))
    return { kind: "malformed" };
  return { kind: "records", records: [{ path, operation: tool === "Write" ? "write" : "update" }] };
}

// packages/toolu-core/src/dispatch/dispatch-bash.ts
import { constants } from "os";

// packages/toolu-core/src/dispatch/dispatch-output.ts
function substituted2(text) {
  return text.replace(/\n+$/, "");
}
function parseDocument(text) {
  try {
    const parsed = JSON.parse(text);
    return parsed;
  } catch {
    return;
  }
}
function jqRaw(value) {
  return typeof value === "string" ? value : toJqJson(value, true);
}
function readField(doc, path) {
  let value = doc;
  for (const key of path) {
    if (value === null)
      return "";
    if (!isJsonObject(value))
      return "";
    value = value[key];
  }
  if (value === undefined || value === null || value === false)
    return "";
  return substituted2(jqRaw(value));
}
function emptyAdvisories() {
  return { contexts: [], messages: [] };
}
function addOnce(list, text) {
  if (text !== "" && !list.includes(text))
    list.push(text);
}
function collectAdvisories(into, doc) {
  addOnce(into.contexts, readField(doc, ["hookSpecificOutput", "additionalContext"]));
  addOnce(into.messages, readField(doc, ["systemMessage"]));
}
function jqPrint(value) {
  return `${toJqJson(value, true)}
`;
}
function printed(result) {
  return `${result}
`;
}
function joined(list) {
  return list.join(`

`);
}
function concat(left, right) {
  return typeof left === "string" ? left + right : undefined;
}
function enriched(ask, ctx, msg) {
  const hso = ask.hookSpecificOutput;
  if (!isJsonObject(hso))
    return;
  const current = hso.permissionDecisionReason;
  const reason = current === undefined || current === null || current === false ? "" : current;
  const nextReason = ctx === "" ? reason : concat(reason, `

${ctx}`);
  if (nextReason === undefined)
    return;
  const out = {
    ...ask,
    hookSpecificOutput: { ...hso, permissionDecisionReason: nextReason }
  };
  if (msg === "")
    return out;
  const sys = ask.systemMessage;
  const base = sys === undefined || sys === null || sys === false ? "" : sys;
  const nextMsg = base === "" ? msg : concat(base, `

${msg}`);
  if (nextMsg === undefined)
    return;
  out.systemMessage = nextMsg;
  return out;
}
function finalAsk(askResult, advisories) {
  const ask = parseDocument(askResult);
  const out = isJsonObject(ask) ? enriched(ask, joined(advisories.contexts), joined(advisories.messages)) : undefined;
  return out === undefined ? printed(askResult) : jqPrint(out);
}
function finalAdvisory(advisories, hookEventName) {
  const ctx = joined(advisories.contexts);
  const msg = joined(advisories.messages);
  if (ctx === "" && msg === "")
    return "";
  const out = {};
  if (ctx !== "")
    out.hookSpecificOutput = { hookEventName, additionalContext: ctx };
  if (msg !== "")
    out.systemMessage = msg;
  return jqPrint(out);
}

// packages/toolu-core/src/dispatch/dispatch-bash.ts
function childEnv2(env, extra) {
  const out = {};
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined)
      out[key] = value;
  }
  return { ...out, ...extra };
}
function shellStatus(proc) {
  if (proc.signalCode === undefined)
    return proc.exitCode;
  const signals = constants.signals;
  return 128 + (signals[proc.signalCode] ?? 0);
}
function runBash(script, stdin, env) {
  try {
    const proc = Bun.spawnSync(["bash", script], {
      stdin: new TextEncoder().encode(stdin),
      stdout: "pipe",
      stderr: "pipe",
      env
    });
    return {
      stdout: substituted2(proc.stdout.toString()),
      stderr: proc.stderr.toString(),
      exitCode: shellStatus(proc)
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { stdout: "", stderr: `${message}
`, exitCode: 127 };
  }
}
function moduleStdin(input) {
  return `${input}
`;
}

// packages/toolu-core/src/decision/decision.ts
var DecisionSchema = discriminatedUnion("kind", [
  object({ kind: literal("allow") }),
  object({ kind: literal("ask"), reason: string2().min(1) }),
  object({ kind: literal("deny"), reason: string2().min(1) }),
  object({ kind: literal("advisory"), message: string2().min(1) }),
  object({ kind: literal("post_block"), reason: string2().min(1) }),
  object({
    kind: literal("runtime_failure"),
    reason: string2().min(1),
    code: _enum(["timeout", "spawn", "parse", "truncated", "cancelled", "nonzero"])
  })
]);

// packages/toolu-core/src/host/host-encode.ts
var PRE_ACTION = new Set(["tool/pre", "shell/pre"]);
var BLOCKING = new Set(["tool/pre", "shell/pre", "permission/evaluate"]);
var CONTEXT_ONLY = new Set([
  "session/start",
  "session/unload",
  "pre_compact"
]);
var ADDITIONAL_CONTEXT = new Set([
  "tool/pre",
  "shell/pre",
  "tool/post",
  "session/start",
  "prompt"
]);
var ASK_EVENTS = {
  claude: new Set(["tool/pre", "shell/pre", "permission/evaluate"]),
  codex: new Set(["permission/evaluate"]),
  cursor: new Set(["shell/pre"]),
  hermes: new Set,
  opencode: new Set
};
function supportsAsk(host, event = "tool/pre") {
  return ASK_EVENTS[host].has(event);
}
function normalize(host, event, decision) {
  if (decision.kind === "runtime_failure") {
    return BLOCKING.has(event) ? { kind: "deny", reason: decision.reason } : { kind: "advisory", message: decision.reason };
  }
  if (decision.kind === "ask" && !supportsAsk(host, event)) {
    return PRE_ACTION.has(event) ? { kind: "deny", reason: decision.reason } : { kind: "advisory", message: decision.reason };
  }
  if (decision.kind === "deny" || decision.kind === "post_block") {
    if (event === "tool/post")
      return { kind: "post_block", reason: decision.reason };
    if (CONTEXT_ONLY.has(event))
      return { kind: "advisory", message: decision.reason };
    return { kind: "deny", reason: decision.reason };
  }
  return decision;
}
function text(decision) {
  return decision.kind === "advisory" ? decision.message : decision.kind === "allow" ? "" : decision.reason;
}
function json(value) {
  return `${JSON.stringify(value)}
`;
}
function hookOutput(native, event, decision) {
  if (decision.kind === "allow")
    return "";
  if (decision.kind === "post_block")
    return json({ decision: "block", reason: decision.reason });
  if (decision.kind === "advisory") {
    return ADDITIONAL_CONTEXT.has(event) ? json({ hookSpecificOutput: { hookEventName: native, additionalContext: decision.message } }) : json({ systemMessage: decision.message });
  }
  if (event === "permission/evaluate") {
    return decision.kind === "deny" ? json({
      hookSpecificOutput: {
        hookEventName: native,
        decision: { behavior: "deny", message: decision.reason }
      }
    }) : "";
  }
  if (event === "prompt")
    return json({ decision: "block", reason: decision.reason });
  return json({
    hookSpecificOutput: {
      hookEventName: native,
      permissionDecision: decision.kind,
      permissionDecisionReason: decision.reason
    }
  });
}
function cursorOutput(event, decision) {
  if (PRE_ACTION.has(event)) {
    if (decision.kind === "deny" || decision.kind === "ask") {
      return json({
        permission: decision.kind,
        user_message: decision.reason,
        agent_message: decision.reason
      });
    }
    return decision.kind === "advisory" ? json({ permission: "allow", agent_message: decision.message }) : json({ permission: "allow" });
  }
  if (event === "prompt") {
    return decision.kind === "deny" ? json({ continue: false, user_message: decision.reason }) : json({ continue: true });
  }
  const hasContext = decision.kind === "advisory" || decision.kind === "post_block";
  if (hasContext && (event === "tool/post" || event === "session/start")) {
    return json({ additional_context: text(decision) });
  }
  return json({});
}
function hermesOutput(event, decision) {
  if (PRE_ACTION.has(event) && decision.kind === "deny") {
    return json({ action: "block", message: decision.reason });
  }
  if (event === "prompt" && (decision.kind === "advisory" || decision.kind === "deny")) {
    return json({ context: text(decision) });
  }
  return "";
}
function opencodeCallback(decision) {
  if (decision.kind === "deny" || decision.kind === "ask") {
    return { kind: "callback", action: "throw", message: decision.reason };
  }
  if (decision.kind === "advisory" || decision.kind === "post_block") {
    return { kind: "callback", action: "continue", message: text(decision) };
  }
  return { kind: "callback", action: "continue" };
}
function encodeDecision(host, event, decision) {
  const native = nativeEventName(host, event);
  if (native === null) {
    throw new Error(`${host} has no native event for ${event}`);
  }
  const normalized = normalize(host, event, decision);
  if (host === "opencode") {
    return opencodeCallback(normalized);
  }
  const stdout = host === "cursor" ? cursorOutput(event, normalized) : host === "hermes" ? hermesOutput(event, normalized) : hookOutput(native, event, normalized);
  return { kind: "command", stdout, stderr: "", exitCode: 0 };
}

// packages/toolu-core/src/registry/registry-run.ts
import { statSync as statSync5 } from "fs";
import { join as join6 } from "path";
import { inspect } from "util";

// packages/toolu-core/src/registry/registry-list.ts
import { readdirSync, statSync as statSync3 } from "fs";
import { join as join3 } from "path";

// packages/toolu-core/src/registry/registry-paths.ts
var EVENT_DIRS = {
  "tool/pre": "pre-tools.d",
  "tool/post": "post-tools.d"
};
var REGISTRY_DIRS = Object.values(EVENT_DIRS);
var SEP = "__";
function registryDirName(event) {
  return EVENT_DIRS[event];
}
function parseRegistryName(base) {
  let kind;
  if (base.endsWith(".js"))
    kind = "esm";
  else if (base.endsWith(".sh"))
    kind = "bash";
  else
    return;
  const stem = base.slice(0, -3);
  const at = stem.indexOf(SEP);
  if (at <= 0)
    return null;
  const spec = stem.slice(0, at);
  if (/\s/u.test(spec))
    return null;
  return { spec, name: stem.slice(at + SEP.length), kind };
}

// packages/toolu-core/src/registry/registry-list.ts
function isFile2(path) {
  try {
    return statSync3(path).isFile();
  } catch {
    return false;
  }
}
function readNames(dir) {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}
function listRegistryDir(dir) {
  const listing = { entries: [], rejected: [] };
  const names = readNames(dir).toSorted((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  for (const file of names) {
    if (file.startsWith("."))
      continue;
    const parsed = parseRegistryName(file);
    const path = join3(dir, file);
    if (parsed === undefined || !isFile2(path))
      continue;
    if (parsed === null)
      listing.rejected.push(file);
    else
      listing.entries.push({ ...parsed, path, file });
  }
  return listing;
}

// packages/toolu-core/src/registry/registry-gate.ts
import { readFileSync as readFileSync4, statSync as statSync4 } from "fs";
import { homedir as homedir2 } from "os";
import { join as join5 } from "path";

// packages/toolu-core/src/host/host-snapshot.ts
import { mkdirSync, readFileSync as readFileSync3, renameSync as renameSync2, rmSync as rmSync2, writeFileSync as writeFileSync2 } from "fs";
import { dirname, join as join4 } from "path";
var ListSchema = looseObject({ installed: array(unknown()) });
var EntrySchema = looseObject({
  pluginId: unknown(),
  name: unknown(),
  marketplaceName: unknown()
});
var SnapshotFileSchema = looseObject({
  version: literal(1),
  status: string2(),
  plugins: array(unknown())
});
function codexPluginSnapshotPath(options = {}) {
  const o = resolveHost(options);
  return envValue(o.env, "TOOLU_CODEX_PLUGIN_SNAPSHOT") ?? join4(configRoot(o), "toolu", "codex-plugins.json");
}
function readSnapshotFile(path) {
  try {
    const parsed = SnapshotFileSchema.safeParse(JSON.parse(readFileSync3(path, "utf8")));
    return parsed.success ? parsed.data : undefined;
  } catch {
    return;
  }
}
function codexPluginInstalled(spec, options = {}) {
  if (spec === "") {
    return "absent";
  }
  const snapshot = readSnapshotFile(codexPluginSnapshotPath(options));
  if (snapshot?.status !== "ready") {
    return "unknown";
  }
  return snapshot.plugins.includes(spec) ? "installed" : "absent";
}

// packages/toolu-core/src/registry/registry-gate.ts
function installedPluginsPath(env) {
  const root = envValue(env, "TOOLU_CONFIG_DIR") ?? envValue(env, "CLAUDE_CONFIG_DIR") ?? join5(envValue(env, "HOME") ?? homedir2(), ".claude");
  return envValue(env, "CLAUDE_PLUGINS_REGISTRY") ?? join5(root, "plugins", "installed_plugins.json");
}
function readInstalledPlugins(path) {
  try {
    if (!statSync4(path).isFile())
      return;
    const parsed = JSON.parse(readFileSync4(path, "utf8"));
    return parsed;
  } catch {
    return;
  }
}
function claudePresence(spec, env) {
  const doc = readInstalledPlugins(installedPluginsPath(env));
  if (!isJsonObject(doc) || !isJsonObject(doc.plugins))
    return "unknown";
  return Object.hasOwn(doc.plugins, spec) ? "installed" : "absent";
}
function pluginPresence(spec, options = {}) {
  if (spec === "")
    return "absent";
  const o = resolveHost(options);
  if (o.host === "codex")
    return codexPluginInstalled(spec, o);
  if (o.host !== "claude")
    return "unknown";
  return claudePresence(spec, o.env);
}
function pluginActive(spec, options = {}) {
  return pluginPresence(spec, options) !== "absent";
}

// packages/toolu-core/src/registry/registry-types.ts
var REGISTRY_EVENTS = ["tool/pre", "tool/post"];
function registryEventFor(type) {
  return type === "tool/post" ? "tool/post" : "tool/pre";
}

// packages/toolu-core/src/registry/registry-run.ts
function stderrLine2(line) {
  process.stderr.write(`${line}
`);
}
function message(error) {
  return error instanceof Error ? error.message : String(error);
}
var ModuleSchema = looseObject({
  spec: string2(),
  name: string2(),
  event: _enum(REGISTRY_EVENTS),
  run: custom((value) => typeof value === "function")
});
function runContract(loaded, entry, event, ctx) {
  const exported = isJsonObject(loaded) ? loaded.default : undefined;
  const parsed = ModuleSchema.safeParse(exported);
  if (!parsed.success)
    throw new Error("default export is not a registry module");
  const { spec, name, event: declared } = parsed.data;
  const want = { spec: entry.spec, name: entry.name, event: registryEventFor(event.type) };
  if (spec !== want.spec || name !== want.name || declared !== want.event) {
    throw new Error(`contract mismatch: exports ${JSON.stringify({ spec, name, event: declared })}, file and directory want ${JSON.stringify(want)}`);
  }
  return parsed.data.run.call(exported, event, ctx);
}
async function importModule(entry) {
  const stat = statSync5(entry.path);
  const loaded = await import(`${entry.path}?v=${String(stat.mtimeMs)}-${String(stat.size)}`);
  return loaded;
}
async function runEsm(entry, event, ctx) {
  const result = await runContract(await importModule(entry), entry, event, ctx);
  const decision = DecisionSchema.safeParse(result);
  if (!decision.success) {
    throw new Error(`invalid decision: ${inspect(result)}`);
  }
  return decision.data;
}
async function timed(entry, work) {
  const start = performance.now();
  try {
    const decision = await work();
    return { entry, status: "decision", decision, ms: performance.now() - start };
  } catch (error) {
    return { entry, status: "error", error: message(error), ms: performance.now() - start };
  }
}
function stops(event, outcome) {
  if (outcome.status !== "decision")
    return false;
  const kind = event.type === "tool/post" ? "post_block" : "deny";
  return outcome.decision.kind === kind;
}
async function outcomeOf(entry, walk) {
  const { event, ctx, fallback } = walk;
  if (!walk.active(entry.spec))
    return { entry, status: "skipped", reason: "inactive" };
  if (entry.kind === "esm")
    return timed(entry, () => runEsm(entry, event, ctx));
  if (walk.esmSpecs.has(entry.spec))
    return { entry, status: "skipped", reason: "shadowed" };
  if (fallback === undefined)
    return { entry, status: "skipped", reason: "bash" };
  return timed(entry, () => fallback(entry, event, ctx));
}
function memoActive(ctx, selectedSpecs) {
  const memo = new Map;
  return (spec) => {
    const known = memo.get(spec);
    if (known !== undefined)
      return known;
    const active = (selectedSpecs === undefined || selectedSpecs.has(spec)) && pluginActive(spec, { env: ctx.env, host: ctx.host });
    memo.set(spec, active);
    return active;
  };
}
async function walkFrom(entries, at, walk, outcomes) {
  const entry = entries[at];
  if (entry === undefined)
    return outcomes;
  const outcome = await outcomeOf(entry, walk);
  outcomes.push(outcome);
  if (outcome.status === "error") {
    walk.warn(`toolu-registry: module ${entry.file} failed: ${outcome.error}; output skipped`);
  }
  return stops(walk.event, outcome) ? outcomes : walkFrom(entries, at + 1, walk, outcomes);
}
async function runRegistry(event, ctx, options = {}) {
  const warn = options.warn ?? stderrLine2;
  const dir = join6(ctx.configRoot, "toolu", registryDirName(registryEventFor(event.type)));
  const { entries, rejected } = listRegistryDir(dir);
  for (const file of rejected) {
    warn(`toolu-registry: registry module ${file} lacks <plugin-spec>__<name> namespace; skipped`);
  }
  const walk = {
    warn,
    event,
    ctx,
    fallback: options.fallback,
    esmSpecs: new Set(entries.filter((e) => e.kind === "esm").map((e) => e.spec)),
    active: memoActive(ctx, options.selectedSpecs)
  };
  return walkFrom(entries, 0, walk, []);
}

// packages/toolu-core/src/dispatch/dispatch-context.ts
function text2(value, fallback) {
  return typeof value === "string" && value !== "" ? value : fallback;
}
function toolEvent(payload, doc, session) {
  const raw = isJsonObject(doc) ? doc : {};
  const input = isJsonObject(raw.tool_input) ? raw.tool_input : {};
  const cwd = text2(raw.cwd, session.projectRoot);
  const base = {
    sessionId: text2(raw.session_id, "unknown"),
    cwd,
    projectRoot: session.projectRoot,
    worktree: session.projectRoot,
    toolCallId: text2(raw.tool_use_id, "unknown"),
    toolName: text2(payload.toolName, "unknown"),
    toolInput: input
  };
  if (session.phase === "post") {
    const output = raw.tool_response ?? raw.tool_output;
    return { ...base, type: "tool/post", ...output === undefined ? {} : { toolOutput: output } };
  }
  const command = input.command;
  if ((payload.toolName === "Bash" || payload.toolName === "Shell") && typeof command === "string" && command !== "") {
    return { ...base, type: "shell/pre", command };
  }
  return { ...base, type: "tool/pre" };
}
function toolContext(payload, doc, session) {
  const edit = payload.edit;
  return {
    host: session.host,
    env: session.env,
    configRoot: session.configRoot,
    projectRoot: session.projectRoot,
    cwd: session.cwd,
    raw: isJsonObject(doc) ? doc : {},
    ...edit === undefined ? {} : { edit }
  };
}

// packages/toolu-core/src/dispatch/dispatch-walk.ts
function newWalkState(phase) {
  return { phase, advisories: emptyAdvisories(), ask: undefined, stderr: [] };
}
function permissionOf(doc) {
  return readField(doc, ["hookSpecificOutput", "permissionDecision"]);
}
function stopsWalk(phase, doc) {
  return phase === "pre" ? permissionOf(doc) === "deny" : readField(doc, ["decision"]) === "block";
}
function consume(state, name, result) {
  if (result.exitCode === 2) {
    return { stdout: "", stderr: state.stderr.join("") + result.stderr, exitCode: 2 };
  }
  if (result.exitCode !== 0) {
    state.stderr.push(`toolu-dispatch: module ${name} exited ${String(result.exitCode)}; output skipped
`);
    return;
  }
  if (result.stdout === "")
    return;
  const doc = parseDocument(result.stdout);
  if (stopsWalk(state.phase, doc)) {
    return { stdout: printed(result.stdout), stderr: state.stderr.join(""), exitCode: 0 };
  }
  if (state.phase === "pre" && permissionOf(doc) === "ask") {
    state.ask ??= result.stdout;
    return;
  }
  collectAdvisories(state.advisories, doc);
  return;
}
function settle(state) {
  const event = state.phase === "pre" ? "PreToolUse" : "PostToolUse";
  const stdout = state.ask === undefined ? finalAdvisory(state.advisories, event) : finalAsk(state.ask, state.advisories);
  return { stdout, stderr: state.stderr.join(""), exitCode: 0 };
}
function moduleEnv(payload, session) {
  const extra = {
    input: payload.text,
    tool_name: payload.toolName,
    TOOLU_LIB_DIR: session.libDir,
    TOOLU_CONFIG_DIR: session.configRoot
  };
  if (payload.edit !== undefined) {
    extra.TOOLU_EDIT_OPERATION = payload.edit.operation;
    extra.TOOLU_EDIT_FROM = payload.edit.from;
    extra.TOOLU_EDIT_MOVED_TO = payload.edit.movedTo;
  }
  return childEnv2(session.env, extra);
}
function encoded(host, type, decision) {
  const target = host === "codex" ? "codex" : "claude";
  const out = encodeDecision(target, type, decision);
  return { stdout: out.kind === "command" ? substituted2(out.stdout) : "", stderr: "", exitCode: 0 };
}
async function runNative(module, event, ctx) {
  try {
    const parsed = DecisionSchema.safeParse(await module.run(event, ctx));
    if (parsed.success)
      return encoded(ctx.host, event.type, parsed.data);
    return { stdout: "", stderr: "", exitCode: 1 };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { stdout: "", stderr: `${message}
`, exitCode: 1 };
  }
}
async function walkBuiltins(builtins, at, walk, state) {
  const module = builtins[at];
  if (module === undefined)
    return;
  const done = consume(state, module.name, await runNative(module, walk.event, walk.ctx));
  return done ?? walkBuiltins(builtins, at + 1, walk, state);
}
function bashFallback(walk, raw) {
  return (entry) => {
    const result = runBash(entry.path, moduleStdin(walk.payload.text), moduleEnv(walk.payload, walk.session));
    raw.set(entry.path, result);
    const phase = walk.session.phase;
    const stops = result.exitCode === 2 || result.exitCode === 0 && stopsWalk(phase, parseDocument(result.stdout));
    const decision = !stops ? { kind: "allow" } : phase === "pre" ? { kind: "deny", reason: "registry module denied" } : { kind: "post_block", reason: "registry module blocked" };
    return Promise.resolve(decision);
  };
}
function outcomeResult(outcome, walk, raw) {
  if (outcome.status !== "decision")
    return;
  if (outcome.entry.kind === "esm")
    return encoded(walk.ctx.host, walk.event.type, outcome.decision);
  return raw.get(outcome.entry.path);
}
async function walkRegistry(walk, state) {
  const raw = new Map;
  const outcomes = await runRegistry(walk.event, walk.ctx, {
    fallback: bashFallback(walk, raw),
    warn: (line) => state.stderr.push(`${line}
`),
    ...walk.session.selectedRegistrySpecs === undefined ? {} : { selectedSpecs: walk.session.selectedRegistrySpecs }
  });
  for (const outcome of outcomes) {
    const result = outcomeResult(outcome, walk, raw);
    const done = result === undefined ? undefined : consume(state, outcome.entry.file, result);
    if (done !== undefined)
      return done;
  }
  return;
}
async function dispatchModules(payload, session, builtins) {
  const doc = parseDocument(payload.text);
  const walk = {
    payload,
    session,
    event: toolEvent(payload, doc, session),
    ctx: toolContext(payload, doc, session)
  };
  const state = newWalkState(session.phase);
  const done = await walkBuiltins(builtins, 0, walk, state) ?? await walkRegistry(walk, state);
  return done ?? settle(state);
}

// packages/toolu-core/src/dispatch/dispatch.ts
var MALFORMED_PATCH_DENY = `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Unable to parse apply_patch file headers; patch blocked so protected-file and quality gates cannot be bypassed."}}
`;
var MALFORMED_PATCH_BLOCK = `{"decision":"block","reason":"Unable to parse apply_patch file headers; per-file post-edit quality checks could not run."}
`;
function syntheticEdit(doc, record) {
  const input = doc.tool_input;
  const base = input === undefined || input === null || input === false ? {} : input;
  return toJqJson({
    ...doc,
    tool_name: "Edit",
    tool_input: {
      ...isJsonObject(base) ? base : {},
      file_path: record.path,
      path: record.path,
      toolu_edit_operation: record.operation,
      toolu_edit_from: record.from ?? "",
      toolu_edit_moved_to: record.moved_to ?? ""
    }
  }, false);
}
async function dispatchRecords(doc, records, at, walk) {
  const record = records[at];
  if (record === undefined) {
    return walk.blocks.length === 0 ? settle(walk.state) : {
      stdout: `${toJqJson({ decision: "block", reason: walk.blocks.join(`

`) }, false)}
`,
      stderr: walk.state.stderr.join(""),
      exitCode: 0
    };
  }
  const edit = {
    operation: record.operation,
    from: record.from ?? "",
    movedTo: record.moved_to ?? ""
  };
  const payload = { text: syntheticEdit(doc, record), toolName: "Edit", edit };
  const result = await dispatchModules(payload, walk.session, walk.builtins);
  walk.state.stderr.push(result.stderr);
  if (walk.continuePostBlocks && result.exitCode !== 0) {
    return { ...result, stderr: walk.state.stderr.join("") };
  }
  if (walk.continuePostBlocks) {
    const parsed = parseDocument(result.stdout);
    if (readField(parsed, ["decision"]) === "block") {
      walk.blocks.push(readField(parsed, ["reason"]) || "check blocked");
      return dispatchRecords(doc, records, at + 1, walk);
    }
  }
  const done = consume(walk.state, "", {
    ...result,
    stderr: "",
    stdout: substituted2(result.stdout)
  });
  return done ?? dispatchRecords(doc, records, at + 1, walk);
}
async function dispatchInput(input, session, builtins, continuePostBlocks) {
  const doc = parseDocument(input);
  const toolName = readField(doc, ["tool_name"]);
  const normalized = normalizeEditRecords(doc, toolName);
  if (normalized.kind === "not-edit") {
    return dispatchModules({ text: input, toolName }, session, builtins);
  }
  if (normalized.kind === "malformed" || normalized.records.length === 0 || !isJsonObject(doc)) {
    const stdout = session.phase === "pre" ? MALFORMED_PATCH_DENY : MALFORMED_PATCH_BLOCK;
    return { stdout, stderr: "", exitCode: 0 };
  }
  const state = newWalkState(session.phase);
  return dispatchRecords(doc, normalized.records, 0, {
    session,
    builtins,
    state,
    continuePostBlocks: session.phase === "post" && continuePostBlocks,
    blocks: []
  });
}
function sessionFor(phase, env, host, options) {
  const root = configRoot({ env, host });
  const cwd = options.cwd ?? process.cwd();
  const base = {
    phase,
    host,
    configRoot: root,
    libDir: options.libDir,
    cwd,
    ...options.selectedRegistrySpecs === undefined ? {} : { selectedRegistrySpecs: options.selectedRegistrySpecs }
  };
  if (phase === "pre") {
    const project = projectRoot({ env, host, cwd }) ?? cwd;
    return { ...base, env: childEnv2(env, { TOOLU_CONFIG_DIR: root }), projectRoot: project };
  }
  const project = gitToplevel(env, cwd) ?? cwd;
  const path = `${project}/node_modules/.bin:${env.PATH ?? ""}`;
  const extra = { TOOLU_CONFIG_DIR: root, PROJECT_ROOT: project, PATH: path };
  return { ...base, env: childEnv2(env, extra), projectRoot: project };
}
async function dispatchHook(phase, stdin, options) {
  const env = options.env ?? process.env;
  const host = detectHost({ env });
  const warnings = [];
  const config = loadConfig({
    env,
    host,
    warn: (line) => warnings.push(`toolu-config: ${line}
`)
  });
  if (!enabled(config, "hooks", phase === "pre" ? "pre-tools" : "post-tools")) {
    return { stdout: "", stderr: warnings.join(""), exitCode: 0 };
  }
  const session = sessionFor(phase, env, host, options);
  const result = await dispatchInput(substituted2(stdin), session, options.builtins, options.continuePostBlocks === true);
  return { ...result, stderr: warnings.join("") + result.stderr };
}
function dispatchPostTool(stdin, options) {
  return dispatchHook("post", stdin, options);
}
// packages/toolu-core/src/config/settings.ts
var CodeEditRulesSchema = object({
  rules: array(object({
    match: string2(),
    docs: array(string2()),
    when_path_matches: array(string2()).optional(),
    extra_docs: array(string2()).optional()
  }).strict())
});

// node_modules/.bun/unbash@4.0.11/node_modules/unbash/dist/ansi-c.js
function isOctal(code) {
  return code >= 48 && code <= 55;
}
function isHex(code) {
  return code >= 48 && code <= 57 || code >= 65 && code <= 70 || code >= 97 && code <= 102;
}
function codePoint(value, fallback) {
  try {
    return String.fromCodePoint(value);
  } catch {
    return fallback;
  }
}
function decodeAnsiCQuoted(source, start, limit) {
  let pos = start;
  let value = "";
  while (pos < limit && source.charCodeAt(pos) !== 39) {
    if (source.charCodeAt(pos) !== 92 || pos + 1 >= limit) {
      const runStart = pos;
      while (pos < limit) {
        const code = source.charCodeAt(pos);
        if (code === 39 || code === 92 && pos + 1 < limit)
          break;
        pos++;
      }
      value += source.slice(runStart, pos);
      continue;
    }
    const escapeStart = pos++;
    const escaped = source[pos++];
    switch (escaped) {
      case "a":
        value += "\x07";
        break;
      case "b":
        value += "\b";
        break;
      case "e":
      case "E":
        value += "\x1B";
        break;
      case "f":
        value += "\f";
        break;
      case "n":
        value += `
`;
        break;
      case "r":
        value += "\r";
        break;
      case "t":
        value += "\t";
        break;
      case "v":
        value += "\v";
        break;
      case "\\":
        value += "\\";
        break;
      case "'":
        value += "'";
        break;
      case '"':
        value += '"';
        break;
      case "?":
        value += "?";
        break;
      case `
`:
        break;
      case "c": {
        const code = pos < limit ? source.charCodeAt(pos) : 39;
        if (code === 39) {
          value += source.slice(escapeStart, pos);
          break;
        }
        pos++;
        if (code === 92) {
          const pair = pos < limit && source.charCodeAt(pos) === 92;
          if (pair)
            pos++;
          value += "\x1C";
          if (!pair && pos < limit) {
            value += source[pos];
            pos++;
          }
          break;
        }
        value += String.fromCharCode(code === 63 ? 127 : code & 31);
        break;
      }
      case "x":
      case "u":
      case "U": {
        const digitsStart = pos;
        const maxDigits = escaped === "x" ? 2 : escaped === "u" ? 4 : 8;
        while (pos < limit && pos - digitsStart < maxDigits && isHex(source.charCodeAt(pos)))
          pos++;
        if (pos === digitsStart) {
          value += `\\${escaped}`;
          break;
        }
        const raw = source.slice(escapeStart, pos);
        value += codePoint(Number.parseInt(source.slice(digitsStart, pos), 16), raw);
        break;
      }
      default: {
        const escapedCode = escaped.charCodeAt(0);
        if (!isOctal(escapedCode)) {
          value += `\\${escaped}`;
          break;
        }
        while (pos < limit && pos - escapeStart - 1 < 3 && isOctal(source.charCodeAt(pos)))
          pos++;
        value += String.fromCharCode(Number.parseInt(source.slice(escapeStart + 1, pos), 8) & 255);
        break;
      }
    }
  }
  const closed = pos < limit;
  if (closed)
    pos++;
  return { value, end: pos, closed };
}

// node_modules/.bun/unbash@4.0.11/node_modules/unbash/dist/chars.js
var CH_TAB = 9;
var CH_NL = 10;
var CH_SPACE = 32;
var CH_BANG = 33;
var CH_DQUOTE = 34;
var CH_HASH = 35;
var CH_DOLLAR = 36;
var CH_PERCENT = 37;
var CH_AMP = 38;
var CH_SQUOTE = 39;
var CH_LPAREN = 40;
var CH_RPAREN = 41;
var CH_STAR = 42;
var CH_PLUS = 43;
var CH_COMMA = 44;
var CH_DASH = 45;
var CH_SLASH = 47;
var CH_0 = 48;
var CH_9 = 57;
var CH_COLON = 58;
var CH_SEMI = 59;
var CH_LT = 60;
var CH_EQ = 61;
var CH_GT = 62;
var CH_QUESTION = 63;
var CH_AT = 64;
var CH_A = 65;
var CH_Z = 90;
var CH_LBRACKET = 91;
var CH_BACKSLASH = 92;
var CH_RBRACKET = 93;
var CH_CARET = 94;
var CH_UNDERSCORE = 95;
var CH_BACKTICK = 96;
var CH_a = 97;
var CH_z = 122;
var CH_LBRACE = 123;
var CH_PIPE = 124;
var CH_RBRACE = 125;
var CH_TILDE = 126;

// node_modules/.bun/unbash@4.0.11/node_modules/unbash/dist/arithmetic.js
function opPrec(op) {
  switch (op) {
    case ",":
      return 1;
    case "=":
    case "+=":
    case "-=":
    case "*=":
    case "/=":
    case "%=":
    case "<<=":
    case ">>=":
    case "&=":
    case "|=":
    case "^=":
      return 2;
    case "||":
      return 4;
    case "&&":
      return 5;
    case "|":
      return 6;
    case "^":
      return 7;
    case "&":
      return 8;
    case "==":
    case "!=":
      return 9;
    case "<":
    case "<=":
    case ">":
    case ">=":
      return 10;
    case "<<":
    case ">>":
      return 11;
    case "+":
    case "-":
      return 12;
    case "*":
    case "/":
    case "%":
      return 13;
    case "**":
      return 14;
    default:
      return -1;
  }
}
function opRightAssoc(op) {
  switch (op) {
    case "=":
    case "+=":
    case "-=":
    case "*=":
    case "/=":
    case "%=":
    case "<<=":
    case ">>=":
    case "&=":
    case "|=":
    case "^=":
    case "**":
      return true;
    default:
      return false;
  }
}
function parseArithmeticExpression(src, offset = 0, collector) {
  let pos = 0;
  const len = src.length;
  const initialCommandCount = collector?.commandExpansions.length ?? 0;
  const initialWordCount = collector?.embeddedWords.length ?? 0;
  function makeWord(start, end, embedded = false) {
    const node = {
      type: "ArithmeticWord",
      pos: start + offset,
      end: end + offset,
      value: src.slice(start, end),
      parts: undefined
    };
    if (embedded)
      collector?.embeddedWords.push(node);
    return node;
  }
  function skipWS() {
    while (pos < len) {
      const c = src.charCodeAt(pos);
      if (c === CH_SPACE || c === CH_TAB || c === CH_NL)
        pos++;
      else
        break;
    }
  }
  function tryReadBinOp() {
    if (pos >= len)
      return null;
    const c = src.charCodeAt(pos);
    const nc = pos + 1 < len ? src.charCodeAt(pos + 1) : 0;
    const nnc = pos + 2 < len ? src.charCodeAt(pos + 2) : 0;
    switch (c) {
      case CH_COMMA:
        pos++;
        return ",";
      case CH_EQ:
        if (nc === CH_EQ) {
          pos += 2;
          return "==";
        }
        pos++;
        return "=";
      case CH_BANG:
        if (nc === CH_EQ) {
          pos += 2;
          return "!=";
        }
        return null;
      case CH_LT:
        if (nc === CH_LT) {
          if (nnc === CH_EQ) {
            pos += 3;
            return "<<=";
          }
          pos += 2;
          return "<<";
        }
        if (nc === CH_EQ) {
          pos += 2;
          return "<=";
        }
        pos++;
        return "<";
      case CH_GT:
        if (nc === CH_GT) {
          if (nnc === CH_EQ) {
            pos += 3;
            return ">>=";
          }
          pos += 2;
          return ">>";
        }
        if (nc === CH_EQ) {
          pos += 2;
          return ">=";
        }
        pos++;
        return ">";
      case CH_PLUS:
        if (nc === CH_EQ) {
          pos += 2;
          return "+=";
        }
        if (nc === CH_PLUS)
          return null;
        pos++;
        return "+";
      case CH_DASH:
        if (nc === CH_EQ) {
          pos += 2;
          return "-=";
        }
        if (nc === CH_DASH)
          return null;
        pos++;
        return "-";
      case CH_STAR:
        if (nc === CH_STAR) {
          pos += 2;
          return "**";
        }
        if (nc === CH_EQ) {
          pos += 2;
          return "*=";
        }
        pos++;
        return "*";
      case CH_SLASH:
        if (nc === CH_EQ) {
          pos += 2;
          return "/=";
        }
        pos++;
        return "/";
      case CH_PERCENT:
        if (nc === CH_EQ) {
          pos += 2;
          return "%=";
        }
        pos++;
        return "%";
      case CH_PIPE:
        if (nc === CH_PIPE) {
          pos += 2;
          return "||";
        }
        if (nc === CH_EQ) {
          pos += 2;
          return "|=";
        }
        pos++;
        return "|";
      case CH_AMP:
        if (nc === CH_AMP) {
          pos += 2;
          return "&&";
        }
        if (nc === CH_EQ) {
          pos += 2;
          return "&=";
        }
        pos++;
        return "&";
      case CH_CARET:
        if (nc === CH_EQ) {
          pos += 2;
          return "^=";
        }
        pos++;
        return "^";
      case CH_QUESTION:
        pos++;
        return "?";
      default:
        return null;
    }
  }
  function parseBinExpr(minPrec) {
    let left = parseUnaryExpr();
    while (true) {
      skipWS();
      if (pos >= len)
        break;
      const saved = pos;
      const op = tryReadBinOp();
      if (!op)
        break;
      if (op === "?") {
        if (3 < minPrec) {
          pos = saved;
          break;
        }
        const consequent = parseBinExpr(1);
        skipWS();
        if (pos < len && src.charCodeAt(pos) === CH_COLON)
          pos++;
        const alternate = parseBinExpr(3);
        left = { type: "ArithmeticTernary", pos: left.pos, end: alternate.end, test: left, consequent, alternate };
        continue;
      }
      const prec = opPrec(op);
      if (prec < minPrec) {
        pos = saved;
        break;
      }
      const nextPrec = opRightAssoc(op) ? prec : prec + 1;
      const right = parseBinExpr(nextPrec);
      left = { type: "ArithmeticBinary", pos: left.pos, end: right.end, operator: op, left, right };
    }
    return left;
  }
  function parseUnaryExpr() {
    skipWS();
    if (pos >= len)
      return makeWord(pos, pos);
    const start = pos;
    const c = src.charCodeAt(pos);
    const nc = pos + 1 < len ? src.charCodeAt(pos + 1) : 0;
    if (c === CH_PLUS && nc === CH_PLUS) {
      pos += 2;
      const operand = parseUnaryExpr();
      return { type: "ArithmeticUnary", pos: start + offset, end: operand.end, operator: "++", operand, prefix: true };
    }
    if (c === CH_DASH && nc === CH_DASH) {
      pos += 2;
      const operand = parseUnaryExpr();
      return { type: "ArithmeticUnary", pos: start + offset, end: operand.end, operator: "--", operand, prefix: true };
    }
    if (c === CH_BANG) {
      pos++;
      const operand = parseUnaryExpr();
      return { type: "ArithmeticUnary", pos: start + offset, end: operand.end, operator: "!", operand, prefix: true };
    }
    if (c === CH_TILDE) {
      pos++;
      const operand = parseUnaryExpr();
      return { type: "ArithmeticUnary", pos: start + offset, end: operand.end, operator: "~", operand, prefix: true };
    }
    if (c === CH_PLUS && nc !== CH_PLUS && nc !== CH_EQ) {
      pos++;
      const operand = parseUnaryExpr();
      return { type: "ArithmeticUnary", pos: start + offset, end: operand.end, operator: "+", operand, prefix: true };
    }
    if (c === CH_DASH && nc !== CH_DASH && nc !== CH_EQ) {
      pos++;
      const operand = parseUnaryExpr();
      return { type: "ArithmeticUnary", pos: start + offset, end: operand.end, operator: "-", operand, prefix: true };
    }
    return parsePostfixExpr();
  }
  function parsePostfixExpr() {
    const operand = parseAtom();
    skipWS();
    if (pos + 1 < len) {
      const c = src.charCodeAt(pos);
      const nc = src.charCodeAt(pos + 1);
      if (c === CH_PLUS && nc === CH_PLUS) {
        pos += 2;
        return { type: "ArithmeticUnary", pos: operand.pos, end: pos + offset, operator: "++", operand, prefix: false };
      }
      if (c === CH_DASH && nc === CH_DASH) {
        pos += 2;
        return { type: "ArithmeticUnary", pos: operand.pos, end: pos + offset, operator: "--", operand, prefix: false };
      }
    }
    return operand;
  }
  function parseAtom() {
    skipWS();
    if (pos >= len)
      return makeWord(pos, pos);
    const c = src.charCodeAt(pos);
    if (c === CH_LPAREN) {
      const start = pos;
      pos++;
      const expr = parseBinExpr(0);
      skipWS();
      if (pos < len && src.charCodeAt(pos) === CH_RPAREN)
        pos++;
      return { type: "ArithmeticGroup", pos: start + offset, end: pos + offset, expression: expr };
    }
    if (c === CH_DOLLAR) {
      const start = pos;
      const commandCount = collector?.commandExpansions.length ?? 0;
      const wordCount = collector?.embeddedWords.length ?? 0;
      const atom = readDollarAtom();
      const wordEnd = collector?.findArithmeticWordEnd?.(start + offset, offset + len) ?? pos + offset;
      if (wordEnd > pos + offset) {
        if (collector) {
          collector.commandExpansions.length = commandCount;
          collector.embeddedWords.length = wordCount;
        }
        pos = wordEnd - offset;
        return makeWord(start, pos, true);
      }
      return atom;
    }
    if (c === 96 || c === 34 || c === 39) {
      const start = pos;
      pos = (collector?.findArithmeticWordEnd?.(start + offset, offset + len) ?? start + offset + 1) - offset;
      return makeWord(start, pos, true);
    }
    const start = pos;
    const wordCount = collector?.embeddedWords.length ?? 0;
    const atom = readWordAtom();
    const wordEnd = collector?.findArithmeticWordEnd?.(start + offset, offset + len) ?? pos + offset;
    if (wordEnd > pos + offset) {
      if (collector)
        collector.embeddedWords.length = wordCount;
      pos = wordEnd - offset;
      return makeWord(start, pos, true);
    }
    return atom;
  }
  function readDollarAtom() {
    const start = pos;
    pos++;
    if (pos >= len)
      return makeWord(start, pos);
    const c = src.charCodeAt(pos);
    if (c === CH_LPAREN) {
      if (pos + 1 < len && src.charCodeAt(pos + 1) === CH_LPAREN) {
        const expansionEnd = collector?.findArithmeticExpansionEnd(start + offset, offset + len) ?? -1;
        if (expansionEnd !== -1) {
          pos = expansionEnd - offset;
        } else {
          pos += 2;
          let depth = 1;
          while (pos < len && depth > 0) {
            if (src.charCodeAt(pos) === CH_LPAREN && src.charCodeAt(pos + 1) === CH_LPAREN) {
              depth++;
              pos += 2;
            } else if (src.charCodeAt(pos) === CH_RPAREN && src.charCodeAt(pos + 1) === CH_RPAREN) {
              depth--;
              pos += 2;
            } else {
              pos++;
            }
          }
        }
      } else {
        pos++;
        const close = collector?.findClosingParenthesis(pos + offset, offset + len) ?? -1;
        if (close !== -1) {
          pos = close - offset + 1;
        } else {
          let depth = 1;
          while (pos < len && depth > 0) {
            const ch = src.charCodeAt(pos++);
            if (ch === CH_LPAREN)
              depth++;
            else if (ch === CH_RPAREN)
              depth--;
          }
        }
        const text = src.slice(start, pos);
        const inner = text.slice(2, -1);
        const node = {
          type: "ArithmeticCommandExpansion",
          pos: start + offset,
          end: pos + offset,
          text,
          inner,
          script: undefined
        };
        collector?.commandExpansions.push(node);
        return node;
      }
    } else if (c === CH_LBRACE) {
      const close = collector?.findClosingBrace(pos + offset + 1, offset + len) ?? -1;
      if (close !== -1) {
        pos = close - offset + 1;
      } else {
        pos++;
        let depth = 1;
        while (pos < len && depth > 0) {
          const ch = src.charCodeAt(pos++);
          if (ch === CH_LBRACE)
            depth++;
          else if (ch === CH_RBRACE)
            depth--;
        }
      }
    } else {
      while (pos < len) {
        const ch = src.charCodeAt(pos);
        if (ch >= CH_a && ch <= CH_z || ch >= CH_A && ch <= CH_Z || ch >= CH_0 && ch <= CH_9 || ch === CH_UNDERSCORE)
          pos++;
        else
          break;
      }
    }
    return makeWord(start, pos, c === CH_LPAREN || c === CH_LBRACE);
  }
  function readWordAtom() {
    const start = pos;
    while (pos < len) {
      const c = src.charCodeAt(pos);
      if (c >= CH_0 && c <= CH_9 || c >= CH_A && c <= CH_Z || c >= CH_a && c <= CH_z || c === CH_UNDERSCORE || c === 35) {
        pos++;
      } else
        break;
    }
    if (pos > start && pos < len && src.charCodeAt(pos) === CH_LBRACKET) {
      const close = collector?.findClosingBracket?.(pos + offset + 1, offset + len) ?? -1;
      if (close !== -1) {
        pos = close - offset + 1;
      } else {
        pos++;
        let depth = 1;
        while (pos < len && depth > 0) {
          const c = src.charCodeAt(pos);
          if (c === CH_LBRACKET)
            depth++;
          else if (c === CH_RBRACKET)
            depth--;
          pos++;
        }
      }
      return makeWord(start, pos, true);
    }
    if (pos === start) {
      pos++;
      return makeWord(start, pos);
    }
    return makeWord(start, pos);
  }
  skipWS();
  if (pos >= len)
    return null;
  const result = parseBinExpr(0);
  skipWS();
  if (pos < len && collector) {
    collector.commandExpansions.length = initialCommandCount;
    collector.embeddedWords.length = initialWordCount;
    return makeWord(0, len, true);
  }
  return result;
}

// node_modules/.bun/unbash@4.0.11/node_modules/unbash/dist/word.js
function dequoteValue(parts) {
  let s = "";
  for (const c of parts)
    s += c.type === "Literal" ? c.value : c.text;
  return s;
}
function unescapeBareValue(text) {
  const first = text.indexOf("\\");
  if (first === -1)
    return text;
  let s = "";
  let start = 0;
  for (let i = first;i < text.length; i++) {
    if (text.charCodeAt(i) !== 92)
      continue;
    s += text.slice(start, i);
    i++;
    if (i >= text.length) {
      s += "\\";
      start = i;
      break;
    }
    if (text.charCodeAt(i) !== 10)
      s += text[i];
    start = i + 1;
  }
  return s + text.slice(start);
}
function commandExpansionValue(text) {
  if (text[0] !== "$")
    return text;
  let pos = 1;
  while (text[pos] === "\\" && text[pos + 1] === `
`)
    pos += 2;
  return pos === 1 || text[pos] !== "(" ? text : "$" + text.slice(pos);
}

class WordImpl {
  static _resolveWord;
  static _resolveHeredocBody;
  text;
  pos;
  end;
  #source;
  #resolver;
  #depth;
  #parts;
  #value = null;
  constructor(text, pos, end, source, resolver, depth = 0) {
    this.text = text;
    this.pos = pos;
    this.end = end;
    this.#source = source;
    this.#resolver = resolver ?? WordImpl._resolveWord;
    this.#depth = depth;
    this.#parts = source !== undefined ? null : undefined;
  }
  get value() {
    if (this.#value === null) {
      const parts = this.parts;
      if (!parts) {
        this.#value = unescapeBareValue(this.text);
      } else {
        let s = "";
        for (const p of parts) {
          switch (p.type) {
            case "Literal":
            case "SingleQuoted":
            case "AnsiCQuoted":
              s += p.value;
              break;
            case "DoubleQuoted":
            case "LocaleString":
              s += dequoteValue(p.parts);
              break;
            case "CommandExpansion":
              s += commandExpansionValue(p.text);
              break;
            default:
              s += p.text;
              break;
          }
        }
        this.#value = s;
      }
    }
    return this.#value;
  }
  get parts() {
    if (this.#parts === null) {
      this.#parts = this.#resolver(this.#source ?? "", this, this.#depth) ?? undefined;
    }
    return this.#parts;
  }
  set parts(v) {
    this.#parts = v ?? undefined;
  }
  sourceText() {
    return this.#source?.slice(this.pos, this.end);
  }
  toJSON() {
    return { text: this.text, pos: this.pos, end: this.end, parts: this.parts, value: this.value };
  }
}

// node_modules/.bun/unbash@4.0.11/node_modules/unbash/dist/lexer.js
var MAX_SYNTAX_NESTING = 256;
var Token = {
  Word: 0,
  Assignment: 1,
  Semi: 2,
  Newline: 3,
  Pipe: 4,
  And: 5,
  Or: 6,
  Amp: 7,
  LParen: 8,
  RParen: 9,
  LBrace: 10,
  RBrace: 11,
  Bang: 12,
  If: 13,
  Then: 14,
  Else: 15,
  Elif: 16,
  Fi: 17,
  Do: 18,
  Done: 19,
  For: 20,
  While: 21,
  Until: 22,
  In: 23,
  Case: 24,
  Esac: 25,
  Function: 26,
  DoubleSemi: 27,
  SemiAmp: 28,
  DoubleSemiAmp: 29,
  Select: 30,
  DblLBracket: 31,
  DblRBracket: 32,
  EOF: 33,
  ArithCmd: 34,
  Coproc: 35,
  Redirect: 36
};

class TokenValue {
  token = Token.EOF;
  _value = "";
  _owner;
  pos = 0;
  end = 0;
  fileDescriptor = undefined;
  variableName = undefined;
  content = undefined;
  targetPos = 0;
  targetEnd = 0;
  assignmentOperatorPos = -1;
  raw = false;
  keywordEligible = false;
  constructor(owner = null) {
    this._owner = owner;
  }
  get value() {
    return this._value ?? (this._value = this._owner === null ? "" : this._owner._tokenValue(this.pos, this.end, this.raw));
  }
  set value(v) {
    this._value = v;
  }
  reset() {
    this.token = Token.EOF;
    this._value = "";
    this.pos = 0;
    this.end = 0;
    this.fileDescriptor = undefined;
    this.variableName = undefined;
    this.content = undefined;
    this.targetPos = 0;
    this.targetEnd = 0;
    this.assignmentOperatorPos = -1;
    this.raw = false;
    this.keywordEligible = false;
  }
  copyFrom(other) {
    this.token = other.token;
    this._value = other._value;
    this.pos = other.pos;
    this.end = other.end;
    this.fileDescriptor = other.fileDescriptor;
    this.variableName = other.variableName;
    this.content = other.content;
    this.targetPos = other.targetPos;
    this.targetEnd = other.targetEnd;
    this.assignmentOperatorPos = other.assignmentOperatorPos;
    this.raw = other.raw;
    this.keywordEligible = other.keywordEligible;
  }
}
var RESERVED_WORDS = new Map([
  ["if", Token.If],
  ["then", Token.Then],
  ["else", Token.Else],
  ["elif", Token.Elif],
  ["fi", Token.Fi],
  ["do", Token.Do],
  ["done", Token.Done],
  ["for", Token.For],
  ["while", Token.While],
  ["until", Token.Until],
  ["in", Token.In],
  ["case", Token.Case],
  ["esac", Token.Esac],
  ["function", Token.Function],
  ["select", Token.Select],
  ["coproc", Token.Coproc],
  ["!", Token.Bang],
  ["{", Token.LBrace],
  ["}", Token.RBrace]
]);
var charType = new Uint8Array(128);
charType[CH_PIPE] = 1;
charType[CH_AMP] = 1;
charType[CH_SEMI] = 1;
charType[CH_LPAREN] = 1;
charType[CH_RPAREN] = 1;
charType[CH_LT] = 1;
charType[CH_GT] = 1;
charType[CH_SPACE] = 1;
charType[CH_TAB] = 1;
charType[CH_NL] = 1;
charType[CH_BACKSLASH] = 2;
charType[CH_SQUOTE] = 2;
charType[CH_DQUOTE] = 2;
charType[CH_DOLLAR] = 2;
charType[CH_BACKTICK] = 2;
charType[CH_LBRACE] = 2;
function skipLineContinuations(source, pos, end) {
  while (pos + 1 < end && source.charCodeAt(pos) === CH_BACKSLASH && source.charCodeAt(pos + 1) === CH_NL)
    pos += 2;
  return pos;
}
var arithmeticWordDelimiter = new Uint8Array(128);
for (const ch of [
  CH_TAB,
  CH_NL,
  CH_SPACE,
  CH_BANG,
  CH_PERCENT,
  CH_AMP,
  CH_LPAREN,
  CH_RPAREN,
  CH_STAR,
  CH_PLUS,
  CH_COMMA,
  CH_DASH,
  CH_SLASH,
  CH_COLON,
  CH_LT,
  CH_EQ,
  CH_GT,
  CH_QUESTION,
  CH_CARET,
  CH_PIPE
]) {
  arithmeticWordDelimiter[ch] = 1;
}
function hasEmbeddedWordStructure(source, start, end) {
  for (let pos = start;pos < end; pos++) {
    const ch = source.charCodeAt(pos);
    if (ch === CH_BACKSLASH || ch === CH_SQUOTE || ch === CH_DQUOTE || ch === CH_DOLLAR || ch === CH_BACKTICK || (ch === CH_LT || ch === CH_GT) && pos + 1 < end && source.charCodeAt(pos + 1) === CH_LPAREN) {
      return true;
    }
  }
  return false;
}
function findUnnested(s, target, pairTernaries = false, findNestedEnd) {
  let depth = 0;
  let ternaryDepth = 0;
  for (let i = 0;i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c === CH_BACKSLASH) {
      i++;
      continue;
    }
    if (c === CH_LBRACE) {
      depth++;
      continue;
    }
    if (c === CH_RBRACE) {
      if (depth > 0)
        depth--;
      continue;
    }
    if (c === CH_SQUOTE) {
      i++;
      while (i < s.length && s.charCodeAt(i) !== CH_SQUOTE)
        i++;
      continue;
    }
    if (c === CH_DQUOTE) {
      i++;
      while (i < s.length) {
        const quoted = s.charCodeAt(i);
        if (quoted === CH_DQUOTE)
          break;
        if (quoted === CH_BACKSLASH) {
          i += 2;
          continue;
        }
        const nestedEnd = findNestedEnd?.(i, true) ?? i;
        if (nestedEnd > i) {
          i = nestedEnd;
          continue;
        }
        i++;
      }
      continue;
    }
    const nestedEnd = findNestedEnd?.(i, false) ?? i;
    if (nestedEnd > i) {
      i = nestedEnd - 1;
      continue;
    }
    if (pairTernaries && depth === 0) {
      if (c === CH_QUESTION) {
        ternaryDepth++;
        continue;
      }
      if (c === CH_COLON && ternaryDepth > 0) {
        ternaryDepth--;
        continue;
      }
    }
    if (c === target && depth === 0)
      return i;
  }
  return -1;
}
var isIdChar = new Uint8Array(128);
for (let i = CH_a;i <= CH_z; i++)
  isIdChar[i] = 3;
for (let i = CH_A;i <= CH_Z; i++)
  isIdChar[i] = 3;
for (let i = CH_0;i <= CH_9; i++)
  isIdChar[i] = 2;
isIdChar[CH_UNDERSCORE] = 3;
var extglobPrefix = new Uint8Array(128);
extglobPrefix[CH_QUESTION] = 1;
extglobPrefix[CH_AT] = 1;
extglobPrefix[CH_STAR] = 1;
extglobPrefix[CH_PLUS] = 1;
extglobPrefix[CH_BANG] = 1;
extglobPrefix[CH_EQ] = 1;
var extglobOp = {
  [CH_QUESTION]: "?",
  [CH_AT]: "@",
  [CH_STAR]: "*",
  [CH_PLUS]: "+",
  [CH_BANG]: "!"
};
function isDQChild(p) {
  const t = p.type;
  return t === "Literal" || t === "SimpleExpansion" || t === "ParameterExpansion" || t === "CommandExpansion" || t === "ArithmeticExpansion";
}
function isAllDigits(text) {
  for (let i = 0;i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (c < CH_0 || c > CH_9)
      return false;
  }
  return text.length > 0;
}
function isAllDigitsRange(src, start, end) {
  for (let i = start;i < end; i++) {
    const c = src.charCodeAt(i);
    if (c < CH_0 || c > CH_9)
      return false;
  }
  return end > start;
}
var ASSIGNMENT_INVALID = -1;
var ASSIGNMENT_NAME_START = 0;
var ASSIGNMENT_NAME = 1;
var ASSIGNMENT_AFTER_INDEX = 2;
var ASSIGNMENT_AFTER_PLUS = 3;
var ASSIGNMENT_INDEX_BASE = 4;
function isMatchedAssignment(state) {
  return state < ASSIGNMENT_INVALID;
}
function assignmentOperatorPos(state) {
  return -state - 2;
}
function scanAssignmentPrefix(src, start, end, initialState) {
  let state = initialState;
  for (let i = start;i < end && state >= 0; i++) {
    const c = src.charCodeAt(i);
    if (state >= ASSIGNMENT_INDEX_BASE) {
      if (c === CH_LBRACKET)
        state++;
      else if (c === CH_RBRACKET && --state === ASSIGNMENT_INDEX_BASE)
        state = ASSIGNMENT_AFTER_INDEX;
    } else if (state === ASSIGNMENT_NAME_START) {
      state = c < 128 && isIdChar[c] & 1 ? ASSIGNMENT_NAME : ASSIGNMENT_INVALID;
    } else if (state === ASSIGNMENT_NAME) {
      if (c < 128 && isIdChar[c] & 2)
        continue;
      if (c === CH_LBRACKET)
        state = ASSIGNMENT_INDEX_BASE + 1;
      else if (c === CH_PLUS)
        state = ASSIGNMENT_AFTER_PLUS;
      else
        state = c === CH_EQ ? -i - 2 : ASSIGNMENT_INVALID;
    } else if (state === ASSIGNMENT_AFTER_INDEX) {
      if (c === CH_PLUS)
        state = ASSIGNMENT_AFTER_PLUS;
      else
        state = c === CH_EQ ? -i - 2 : ASSIGNMENT_INVALID;
    } else {
      state = c === CH_EQ ? -i - 2 : ASSIGNMENT_INVALID;
    }
  }
  return state;
}
var NO_EXPANSIONS = [];
function setToken(out, token, value, pos = 0, end = 0) {
  out.token = token;
  out._value = value;
  out.pos = pos;
  out.end = end;
  out.fileDescriptor = undefined;
  out.variableName = undefined;
  out.content = undefined;
  out.assignmentOperatorPos = -1;
  out.raw = false;
  out.keywordEligible = false;
}
function setSpanToken(out, token, pos, end, raw) {
  out.token = token;
  out._value = null;
  out.pos = pos;
  out.end = end;
  out.fileDescriptor = undefined;
  out.variableName = undefined;
  out.content = undefined;
  out.assignmentOperatorPos = -1;
  out.raw = raw;
  out.keywordEligible = false;
}
function matchReservedWord(src, start, len) {
  switch (src.charCodeAt(start)) {
    case CH_BANG:
      return len === 1 ? Token.Bang : undefined;
    case CH_LBRACE:
      return len === 1 ? Token.LBrace : undefined;
    case CH_RBRACE:
      return len === 1 ? Token.RBrace : undefined;
    case 105: {
      if (len !== 2)
        return;
      const c = src.charCodeAt(start + 1);
      return c === 102 ? Token.If : c === 110 ? Token.In : undefined;
    }
    case 102:
      if (len === 2)
        return src.charCodeAt(start + 1) === 105 ? Token.Fi : undefined;
      if (len === 3)
        return src.startsWith("for", start) ? Token.For : undefined;
      if (len === 8)
        return src.startsWith("function", start) ? Token.Function : undefined;
      return;
    case 116:
      return len === 4 && src.startsWith("then", start) ? Token.Then : undefined;
    case 101:
      if (len !== 4)
        return;
      if (src.startsWith("else", start))
        return Token.Else;
      if (src.startsWith("elif", start))
        return Token.Elif;
      if (src.startsWith("esac", start))
        return Token.Esac;
      return;
    case 100:
      if (len === 2)
        return src.charCodeAt(start + 1) === 111 ? Token.Do : undefined;
      if (len === 4)
        return src.startsWith("done", start) ? Token.Done : undefined;
      return;
    case 99:
      if (len === 4)
        return src.startsWith("case", start) ? Token.Case : undefined;
      if (len === 6)
        return src.startsWith("coproc", start) ? Token.Coproc : undefined;
      return;
    case 119:
      return len === 5 && src.startsWith("while", start) ? Token.While : undefined;
    case 117:
      return len === 5 && src.startsWith("until", start) ? Token.Until : undefined;
    case 115:
      return len === 6 && src.startsWith("select", start) ? Token.Select : undefined;
    default:
      return;
  }
}
var LexContext = {
  Normal: 0,
  CommandStart: 1,
  TestMode: 2,
  CommandPrefix: 3
};
function scanBraceExpansion(src, pos, len) {
  const nextCh = pos + 1 < len ? src.charCodeAt(pos + 1) : 0;
  if (nextCh <= CH_SPACE || nextCh === CH_RBRACE)
    return -1;
  let depth = 1;
  let hasSep = false;
  let scanPos = pos + 1;
  while (scanPos < len && depth > 0) {
    const bc = src.charCodeAt(scanPos);
    if (bc === CH_LBRACE)
      depth++;
    else if (bc === CH_RBRACE) {
      if (--depth === 0)
        break;
    } else if (bc <= CH_SPACE || bc === CH_SEMI || bc === CH_PIPE || bc === CH_AMP)
      return -1;
    else if (depth === 1 && (bc === 44 || bc === 46 && scanPos + 1 < len && src.charCodeAt(scanPos + 1) === 46))
      hasSep = true;
    if (bc === CH_BACKSLASH)
      scanPos++;
    scanPos++;
  }
  if (depth === 0 && hasSep)
    return scanPos + 1;
  return -1;
}

class Lexer {
  src;
  srcEnd;
  pos;
  current;
  nextState;
  hasPeek;
  pendingHereDocs;
  collectedExpansions;
  _errors = null;
  _buildParts = false;
  _buildValue = false;
  _nestingDepth = 0;
  constructor(src, start = 0, end = src.length) {
    this.src = src;
    this.srcEnd = end;
    this.pos = start;
    this.current = new TokenValue(this);
    this.nextState = new TokenValue(this);
    this.hasPeek = false;
    this.pendingHereDocs = null;
    this.collectedExpansions = null;
    if (start === 0 && src.charCodeAt(0) === CH_HASH && src.charCodeAt(1) === CH_BANG) {
      const nl = src.indexOf(`
`);
      this.pos = nl === -1 ? this.srcEnd : nl + 1;
    }
  }
  getSource() {
    return this.src;
  }
  get errors() {
    return this._errors ?? (this._errors = []);
  }
  getCollectedExpansions() {
    return this.collectedExpansions ?? NO_EXPANSIONS;
  }
  collect(part) {
    (this.collectedExpansions ??= []).push([part, this._nestingDepth]);
  }
  getPos() {
    return this.pos;
  }
  _tokenValue(pos, end, raw) {
    return raw ? this.src.slice(pos, end) : this.wordValueOf(pos, end);
  }
  wordValueOf(start, end) {
    const savedPos = this.pos;
    const savedEnd = this.srcEnd;
    const savedBuildValue = this._buildValue;
    const savedUnbalanced = this._unbalanced;
    const errorCount = this._errors === null ? 0 : this._errors.length;
    this.pos = start;
    this.srcEnd = end;
    this._buildValue = true;
    this.readWordText();
    const value = this._wordText;
    this.pos = savedPos;
    this.srcEnd = savedEnd;
    this._buildValue = savedBuildValue;
    this._unbalanced = savedUnbalanced;
    if (this._errors !== null)
      this._errors.length = errorCount;
    return value;
  }
  findClosingBracket(start, end = this.srcEnd) {
    return this.findClosingShellDelimiter(start, end, CH_RBRACKET);
  }
  findClosingArithmeticBracket(start, end = this.srcEnd) {
    return this.findClosingShellDelimiter(start, end, CH_RBRACKET, false, false);
  }
  findClosingBrace(start, end = this.srcEnd) {
    return this.findClosingShellDelimiter(start, end, CH_RBRACE);
  }
  findClosingParenthesis(start, end = this.srcEnd) {
    const savedPos = this.pos;
    const savedEnd = this.srcEnd;
    const savedUnbalanced = this._unbalanced;
    this.pos = start;
    this.srcEnd = Math.min(end, this.srcEnd);
    this.extractBalanced();
    const close = this._unbalanced ? -1 : this.pos - 1;
    this.pos = savedPos;
    this.srcEnd = savedEnd;
    this._unbalanced = savedUnbalanced;
    return close;
  }
  findArithmeticExpansionEnd(start, end = this.srcEnd) {
    const scanner = new Lexer(this.src, start, end);
    scanner.pos = start + 1;
    scanner.scanArithmeticBody();
    return scanner.pos;
  }
  findArithmeticWordEnd(start, end = this.srcEnd) {
    const scanner = new Lexer(this.src, start, end);
    scanner.pos = start;
    return scanner.scanArithmeticWordEnd();
  }
  scanArithmeticWordEnd() {
    while (this.pos < this.srcEnd) {
      const ch = this.src.charCodeAt(this.pos);
      if (ch === CH_DOLLAR) {
        this.readDollar();
        continue;
      }
      if (ch === CH_BACKTICK) {
        this.readBacktickExpansion();
        continue;
      }
      if (ch === CH_SQUOTE) {
        this.pos++;
        this.skipSQ();
        continue;
      }
      if (ch === CH_DQUOTE) {
        this.pos++;
        this.skipDQ();
        continue;
      }
      if (ch === CH_BACKSLASH) {
        this.pos += 2;
        continue;
      }
      if (ch === CH_LBRACKET) {
        const close = this.findClosingBracket(this.pos + 1);
        if (close !== -1) {
          this.pos = close + 1;
          continue;
        }
      }
      if ((ch === CH_LT || ch === CH_GT) && this.src.charCodeAt(this.pos + 1) === CH_LPAREN) {
        this.pos += 2;
        this.extractBalanced();
        continue;
      }
      if (ch < 128 && arithmeticWordDelimiter[ch])
        break;
      this.pos++;
    }
    return this.pos;
  }
  findClosingShellDelimiter(start, end, closing, comments = false, braces = true) {
    const savedPos = this.pos;
    const savedEnd = this.srcEnd;
    const savedUnbalanced = this._unbalanced;
    this.srcEnd = Math.min(end, this.srcEnd);
    const delimiters = [closing];
    let pos = start;
    let wordStart = true;
    while (pos < this.srcEnd) {
      const ch = this.src.charCodeAt(pos);
      if (ch === CH_BACKSLASH) {
        if (pos + 1 < this.srcEnd && this.src.charCodeAt(pos + 1) !== CH_NL)
          wordStart = false;
        pos += 2;
        continue;
      }
      if (ch === CH_HASH && comments && delimiters.length === 1 && wordStart) {
        while (pos < this.srcEnd && this.src.charCodeAt(pos) !== CH_NL)
          pos++;
        continue;
      }
      if (ch === CH_SQUOTE) {
        this.pos = pos + 1;
        this.skipSQ();
        pos = this.pos;
        wordStart = false;
        continue;
      }
      if (ch === CH_DQUOTE) {
        this.pos = pos + 1;
        this.skipDQ();
        pos = this.pos;
        wordStart = false;
        continue;
      }
      if (ch === CH_BACKTICK) {
        pos++;
        while (pos < this.srcEnd && this.src.charCodeAt(pos) !== CH_BACKTICK) {
          if (this.src.charCodeAt(pos) === CH_BACKSLASH)
            pos++;
          pos++;
        }
        if (pos < this.srcEnd)
          pos++;
        wordStart = false;
        continue;
      }
      if (ch === CH_DOLLAR && pos + 1 < this.srcEnd && this.src.charCodeAt(pos + 1) === CH_LPAREN || (ch === CH_LT || ch === CH_GT) && pos + 1 < this.srcEnd && this.src.charCodeAt(pos + 1) === CH_LPAREN) {
        this.pos = pos + 2;
        this.extractBalanced();
        pos = this.pos;
        wordStart = false;
        continue;
      }
      const expected = delimiters[delimiters.length - 1];
      if (ch === CH_DOLLAR && pos + 1 < this.srcEnd) {
        const after = this.src.charCodeAt(pos + 1);
        if (after === CH_DOLLAR) {
          pos += 2;
          wordStart = false;
          continue;
        }
        if (after === CH_LBRACE && braces) {
          delimiters.push(CH_RBRACE);
          pos += 2;
          wordStart = false;
          continue;
        }
      }
      if (expected === CH_RBRACKET && ch === CH_LBRACKET) {
        delimiters.push(CH_RBRACKET);
      } else if (expected === CH_RPAREN && ch === CH_LPAREN) {
        delimiters.push(CH_RPAREN);
      } else if (ch === expected) {
        delimiters.pop();
        if (delimiters.length === 0) {
          this.pos = savedPos;
          this.srcEnd = savedEnd;
          this._unbalanced = savedUnbalanced;
          return pos;
        }
        wordStart = false;
        pos++;
        continue;
      }
      wordStart = ch < 128 && (charType[ch] & 1) !== 0;
      pos++;
    }
    this.pos = savedPos;
    this.srcEnd = savedEnd;
    this._unbalanced = savedUnbalanced;
    return -1;
  }
  skipSubshellBody() {
    this.extractBalanced();
    return this._unbalanced ? -1 : this.pos;
  }
  skipCompoundBody(closeToken) {
    const frames = [
      { close: closeToken, phase: closeToken === Token.Esac ? "case-pattern" : "commands" }
    ];
    let commandStart = true;
    for (;; ) {
      const value = this.next(commandStart ? LexContext.CommandStart : LexContext.Normal);
      const token = value.token;
      if (token === Token.EOF)
        return -1;
      const last = frames.length - 1;
      const frame = frames[last];
      if (frame.phase === "function-name") {
        if (token === Token.Newline)
          continue;
        frame.phase = "function-body";
        commandStart = true;
        continue;
      } else if (frame.phase === "function-body") {
        if (token === Token.Newline)
          continue;
        frame.phase = "commands";
        commandStart = true;
        if (token === Token.LParen && this.peek(LexContext.Normal).token === Token.RParen) {
          this.next(LexContext.Normal);
          frame.phase = "function-body";
          continue;
        }
      } else if (frame.phase === "coproc-command") {
        if (token === Token.Newline)
          continue;
        if (token === Token.Word) {
          frame.phase = "coproc-body";
          commandStart = true;
          continue;
        }
        frame.phase = "commands";
        commandStart = true;
      } else if (frame.phase === "coproc-body") {
        if (token === Token.Newline)
          continue;
        frame.phase = token === Token.Word && value.keywordEligible && value.value === "time" ? "time-command" : "commands";
        commandStart = true;
        if (frame.phase === "time-command")
          continue;
      } else if (frame.phase === "time-command") {
        if (token === Token.Word && value.keywordEligible && value.value === "-p") {
          frame.phase = "time-command-after-p";
          continue;
        }
        if (token === Token.Word && value.keywordEligible && value.value === "--") {
          frame.phase = "commands";
          continue;
        }
        frame.phase = "commands";
        commandStart = true;
      } else if (frame.phase === "time-command-after-p") {
        if (token === Token.Word && value.keywordEligible && value.value === "--") {
          frame.phase = "commands";
          continue;
        }
        frame.phase = "commands";
        commandStart = true;
      } else if (frame.phase === "for-header") {
        if (token === Token.ArithCmd || token === Token.Semi || token === Token.Newline) {
          commandStart = true;
          continue;
        }
        if (token === Token.Do || token === Token.LBrace) {
          frame.close = token === Token.Do ? Token.Done : Token.RBrace;
          frame.phase = "commands";
          commandStart = true;
          continue;
        }
      } else if (frame.phase === "case-word") {
        if (token === Token.Newline)
          continue;
        frame.phase = "case-in";
        commandStart = false;
        continue;
      } else if (frame.phase === "case-in") {
        if (token === Token.Newline) {
          commandStart = true;
          continue;
        }
        frame.phase = "case-pattern";
        commandStart = true;
        continue;
      } else if (frame.phase === "case-pattern") {
        if (token === Token.Esac && commandStart) {
          frames.pop();
          if (frames.length === 0)
            return value.end;
          commandStart = false;
          continue;
        }
        if (token === Token.RParen) {
          frame.phase = "commands";
          commandStart = true;
        } else {
          commandStart = token === Token.Newline;
        }
        continue;
      }
      if (token === frame.close) {
        frames.pop();
        if (frames.length === 0)
          return value.end;
        commandStart = false;
        continue;
      }
      if (commandStart) {
        switch (token) {
          case Token.LParen:
            frames.push({ close: Token.RParen, phase: "commands" });
            break;
          case Token.LBrace:
            frames.push({ close: Token.RBrace, phase: "commands" });
            break;
          case Token.If:
            frames.push({ close: Token.Fi, phase: "commands" });
            break;
          case Token.For:
            frames.push({ close: Token.Done, phase: "for-header" });
            break;
          case Token.While:
          case Token.Until:
          case Token.Select:
            frames.push({ close: Token.Done, phase: "commands" });
            break;
          case Token.Case:
            frames.push({ close: Token.Esac, phase: "case-word" });
            break;
          case Token.DblLBracket:
            if (!this.skipTestCommandBody())
              return -1;
            commandStart = false;
            continue;
          case Token.Assignment:
          case Token.Redirect:
          case Token.Bang:
          case Token.Then:
          case Token.Else:
          case Token.Elif:
          case Token.Do:
          case Token.In:
            break;
          case Token.Semi:
          case Token.Newline:
          case Token.Pipe:
          case Token.And:
          case Token.Or:
          case Token.Amp:
          case Token.DoubleSemi:
          case Token.SemiAmp:
          case Token.DoubleSemiAmp:
            break;
          case Token.Function:
            frame.phase = "function-name";
            break;
          case Token.Coproc:
            frame.phase = "coproc-command";
            break;
          default:
            if (token === Token.Word && value.keywordEligible && value.value === "time") {
              frame.phase = "time-command";
              commandStart = true;
            } else {
              commandStart = false;
            }
            continue;
        }
      }
      switch (token) {
        case Token.Semi:
        case Token.Newline:
        case Token.Pipe:
        case Token.And:
        case Token.Or:
        case Token.Amp:
          commandStart = true;
          break;
        case Token.DoubleSemi:
        case Token.SemiAmp:
        case Token.DoubleSemiAmp:
          if (frame.close === Token.Esac)
            frame.phase = "case-pattern";
          commandStart = true;
          break;
        case Token.RParen:
          commandStart = true;
          break;
      }
    }
  }
  skipTestGroup() {
    let depth = 1;
    for (;; ) {
      const value = this.next(LexContext.TestMode);
      if (value.token === Token.EOF)
        return -1;
      if (value.token === Token.DblRBracket) {
        this.unshift(value);
        return -1;
      }
      if (value.token === Token.LParen)
        depth++;
      else if (value.token === Token.RParen && --depth === 0)
        return value.end;
    }
  }
  skipTestCommandBody() {
    for (;; ) {
      const token = this.next(LexContext.TestMode).token;
      if (token === Token.DblRBracket)
        return true;
      if (token === Token.EOF)
        return false;
    }
  }
  buildWordParts(startPos) {
    this._buildParts = true;
    this.pos = startPos;
    const ch = this.src.charCodeAt(startPos);
    if ((ch === 60 || ch === 62) && startPos + 1 < this.srcEnd && this.src.charCodeAt(startPos + 1) === 40) {
      this.pos = startPos + 2;
      const inner = this.extractBalanced();
      if (this._unbalanced)
        this.errors.push({ message: "unterminated process substitution", pos: startPos });
      const text = this.src.slice(startPos, this.pos);
      const part = {
        type: "ProcessSubstitution",
        text,
        operator: ch === 60 ? "<" : ">",
        script: undefined,
        inner: inner ?? undefined,
        innerStart: startPos + 2
      };
      this.collect(part);
      if (this.pos < this.srcEnd) {
        this.readWordText();
        if (this._wordParts) {
          this._wordParts.unshift(part);
        } else {
          this._wordParts = [part];
        }
      } else {
        this._wordParts = [part];
      }
    } else {
      this.readWordText();
    }
    return this._wordParts;
  }
  buildEmbeddedWordParts(startPos) {
    this._buildParts = true;
    this.pos = startPos;
    this.readInnerWordText();
    return this._wordParts;
  }
  buildHereDocParts(bodyPos, bodyEnd) {
    this._buildParts = true;
    const src = this.src;
    const parts = [];
    let litBuf = "";
    let litStart = bodyPos;
    let i = bodyPos;
    const flushLit = () => {
      if (litBuf) {
        parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, i) });
        litBuf = "";
      }
    };
    while (i < bodyEnd) {
      const ch = src.charCodeAt(i);
      if (ch === 92) {
        if (i + 1 < bodyEnd) {
          const nc = src.charCodeAt(i + 1);
          if (nc === 36 || nc === 96 || nc === 92) {
            litBuf += String.fromCharCode(nc);
            i += 2;
            continue;
          }
        }
        litBuf += "\\";
        i++;
        continue;
      }
      if (ch === 36) {
        flushLit();
        litStart = i;
        this.pos = i;
        this.readDollar();
        if (this._resultPart) {
          parts.push(this._resultPart);
          litStart = this.pos;
        } else {
          litBuf += src.slice(i, this.pos);
        }
        i = this.pos;
        continue;
      }
      if (ch === 96) {
        flushLit();
        litStart = i;
        this.pos = i;
        this.readBacktickExpansion();
        if (this._resultPart) {
          parts.push(this._resultPart);
          litStart = this.pos;
        } else {
          litBuf += src.slice(i, this.pos);
        }
        i = this.pos;
        continue;
      }
      litBuf += src[i];
      i++;
    }
    flushLit();
    return parts.length > 1 || parts.length === 1 && parts[0].type !== "Literal" ? parts : null;
  }
  registerHereDocTarget(target) {
    if (this.pendingHereDocs === null)
      return;
    for (const hd of this.pendingHereDocs) {
      if (!hd.target) {
        hd.target = target;
        return;
      }
    }
  }
  readTestRegexWord() {
    this.hasPeek = false;
    this.skipSpacesAndTabs();
    const src = this.src;
    const len = this.srcEnd;
    const start = this.pos;
    let depth = 0;
    while (this.pos < len) {
      const ch = src.charCodeAt(this.pos);
      if (ch === CH_LPAREN) {
        depth++;
        this.pos++;
        continue;
      }
      if (ch === CH_BACKSLASH) {
        this.pos += this.pos + 1 < len ? 2 : 1;
        continue;
      }
      if (ch === CH_SQUOTE) {
        const quotePos = this.pos++;
        const ansiC = quotePos > start && src.charCodeAt(quotePos - 1) === CH_DOLLAR;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_SQUOTE) {
          if (ansiC && src.charCodeAt(this.pos) === CH_BACKSLASH && this.pos + 1 < len)
            this.pos++;
          this.pos++;
        }
        if (this.pos < len)
          this.pos++;
        else
          this.errors.push({
            message: ansiC ? "unterminated ANSI-C quote" : "unterminated single quote",
            pos: quotePos
          });
        continue;
      }
      if (ch === CH_DQUOTE) {
        this.pos++;
        this.readDoubleQuoted();
        continue;
      }
      if (ch === CH_BACKTICK) {
        this.readBacktickExpansion();
        continue;
      }
      if (depth > 0) {
        if (ch === CH_RPAREN)
          depth--;
        this.pos++;
        continue;
      }
      if (ch === CH_DOLLAR) {
        this.readDollar();
        continue;
      }
      if ((ch === CH_LT || ch === CH_GT) && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_LPAREN) {
        const subPos = this.pos;
        this.pos += 2;
        this.extractBalanced();
        if (this._unbalanced)
          this.errors.push({ message: "unterminated process substitution", pos: subPos });
        continue;
      }
      if (ch < 128 && charType[ch] & 1 && ch !== CH_PIPE)
        break;
      this.pos++;
    }
    setToken(this.current, Token.Word, src.slice(start, this.pos), start, this.pos);
    return this.current;
  }
  readCStyleForExprs() {
    this.hasPeek = false;
    const src = this.src;
    const len = this.srcEnd;
    while (this.pos < len && (src.charCodeAt(this.pos) === CH_SPACE || src.charCodeAt(this.pos) === CH_TAB))
      this.pos++;
    if (this.pos < len && src.charCodeAt(this.pos) === CH_LPAREN)
      this.pos++;
    const starts = [this.pos, 0, 0];
    const parts = ["", "", "", 0, 0, 0];
    let partIdx = 0;
    let depth = 1;
    let partStart = this.pos;
    while (this.pos < len && depth > 0) {
      const c = src.charCodeAt(this.pos);
      if (c === CH_LPAREN) {
        depth++;
        this.pos++;
      } else if (c === CH_RPAREN) {
        depth--;
        if (depth === 0) {
          const raw = src.slice(partStart, this.pos);
          parts[partIdx] = raw.trim();
          parts[3 + partIdx] = starts[partIdx] + raw.length - raw.trimStart().length;
          this.pos++;
          while (this.pos < len && (src.charCodeAt(this.pos) === CH_SPACE || src.charCodeAt(this.pos) === CH_TAB))
            this.pos++;
          if (this.pos < len && src.charCodeAt(this.pos) === CH_RPAREN)
            this.pos++;
          break;
        }
        this.pos++;
      } else if (c === CH_SEMI && depth === 1) {
        const raw = src.slice(partStart, this.pos);
        parts[partIdx] = raw.trim();
        parts[3 + partIdx] = starts[partIdx] + raw.length - raw.trimStart().length;
        if (partIdx < 2)
          partIdx++;
        this.pos++;
        partStart = this.pos;
        starts[partIdx] = partStart;
      } else if (c === CH_SQUOTE) {
        this.pos++;
        this.skipSQ();
      } else if (c === CH_DQUOTE) {
        this.pos++;
        this.skipDQ();
      } else {
        this.pos++;
      }
    }
    return parts;
  }
  peek(ctx = LexContext.Normal) {
    if (!this.hasPeek) {
      this.readNext(this.nextState, ctx);
      this.hasPeek = true;
    }
    return this.nextState;
  }
  peekFollow(closers) {
    if (!this.hasPeek) {
      const ctx = closers[this.current.token] ? LexContext.CommandStart : LexContext.Normal;
      this.readNext(this.nextState, ctx);
      this.hasPeek = true;
    }
    return this.nextState;
  }
  next(ctx = LexContext.Normal) {
    if (this.hasPeek) {
      this.hasPeek = false;
      const temp = this.current;
      this.current = this.nextState;
      this.nextState = temp;
      return this.current;
    }
    this.readNext(this.current, ctx);
    return this.current;
  }
  unshift(tok) {
    this.nextState.copyFrom(tok);
    this.hasPeek = true;
  }
  readNext(out, ctx) {
    const src = this.src;
    const len = this.srcEnd;
    let pos = this.pos;
    while (pos < len) {
      const ch = src.charCodeAt(pos);
      if (ch === CH_SPACE || ch === CH_TAB) {
        pos++;
        continue;
      }
      if (ch === CH_BACKSLASH && pos + 1 < len && src.charCodeAt(pos + 1) === CH_NL) {
        pos += 2;
        continue;
      }
      if (ch === CH_NL && ctx === LexContext.TestMode) {
        pos++;
        continue;
      }
      break;
    }
    this.pos = pos;
    if (pos >= len) {
      this.consumePendingHereDocs();
      setToken(out, Token.EOF, "", pos, pos);
      return;
    }
    const tokenStart = pos;
    const ch = src.charCodeAt(pos);
    if (ch === CH_HASH) {
      while (this.pos < len && src.charCodeAt(this.pos) !== CH_NL)
        this.pos++;
      this.readNext(out, ctx);
      return;
    }
    if (ch === CH_NL) {
      this.pos++;
      this.consumePendingHereDocs();
      setToken(out, Token.Newline, `
`, tokenStart, this.pos);
      return;
    }
    if (ctx === LexContext.TestMode && (ch === CH_LT || ch === CH_GT) && !(this.pos + 1 < this.srcEnd && src.charCodeAt(this.pos + 1) === CH_LPAREN)) {
      this.pos++;
      setToken(out, Token.Word, ch === CH_LT ? "<" : ">", tokenStart, this.pos);
      out.keywordEligible = true;
      return;
    }
    if (ch < 128 && charType[ch] & 1 && this.tryReadOperator(out, ch, ctx, tokenStart))
      return;
    this.readWord(out, ctx, tokenStart);
  }
  tryReadOperator(out, ch, ctx, tokenStart) {
    const src = this.src;
    const pos = this.pos;
    const next = pos + 1 < this.srcEnd ? src.charCodeAt(pos + 1) : 0;
    switch (ch) {
      case CH_SEMI:
        if (next === CH_SEMI) {
          if (pos + 2 < this.srcEnd && src.charCodeAt(pos + 2) === CH_AMP) {
            this.pos += 3;
            setToken(out, Token.DoubleSemiAmp, ";;&", tokenStart, this.pos);
            return true;
          }
          this.pos += 2;
          setToken(out, Token.DoubleSemi, ";;", tokenStart, this.pos);
          return true;
        }
        if (next === CH_AMP) {
          this.pos += 2;
          setToken(out, Token.SemiAmp, ";&", tokenStart, this.pos);
          return true;
        }
        this.pos++;
        setToken(out, Token.Semi, ";", tokenStart, this.pos);
        return true;
      case CH_PIPE:
        if (next === CH_PIPE) {
          this.pos += 2;
          setToken(out, Token.Or, "||", tokenStart, this.pos);
          return true;
        }
        if (next === CH_AMP) {
          this.pos += 2;
          setToken(out, Token.Pipe, "|&", tokenStart, this.pos);
          return true;
        }
        this.pos++;
        setToken(out, Token.Pipe, "|", tokenStart, this.pos);
        return true;
      case CH_AMP:
        if (next === CH_AMP) {
          this.pos += 2;
          setToken(out, Token.And, "&&", tokenStart, this.pos);
          return true;
        }
        if (next === CH_GT) {
          this.pos += 2;
          const append = this.pos < this.srcEnd && src.charCodeAt(this.pos) === CH_GT;
          if (append)
            this.pos++;
          this.skipSpacesAndTabs();
          const targetPos = this.pos;
          if (this.pos < this.srcEnd && src.charCodeAt(this.pos) !== CH_NL && src.charCodeAt(this.pos) !== CH_HASH) {
            this.readRedirectTargetText();
          }
          this.redirectToken(out, append ? "&>>" : "&>", tokenStart, targetPos);
          return true;
        }
        this.pos++;
        setToken(out, Token.Amp, "&", tokenStart, this.pos);
        return true;
      case CH_LPAREN:
        if (ctx === LexContext.CommandStart && next === CH_LPAREN) {
          const savedErrors = this.errors.length;
          this.readArithmeticCommand(out, tokenStart);
          if (!this._notArithmetic)
            return true;
          this.errors.length = savedErrors;
          this.pos = tokenStart;
        }
        this.pos++;
        setToken(out, Token.LParen, "(", tokenStart, this.pos);
        return true;
      case CH_RPAREN:
        this.pos++;
        setToken(out, Token.RParen, ")", tokenStart, this.pos);
        return true;
      case CH_LT:
      case CH_GT:
        return this.readRedirection(out, tokenStart);
      default:
        return false;
    }
  }
  readRedirection(out, tokenStart) {
    const src = this.src;
    const ch = src.charCodeAt(this.pos);
    let op = "";
    if (ch === CH_LT) {
      this.pos++;
      const next = this.pos < this.srcEnd ? src.charCodeAt(this.pos) : 0;
      if (next === CH_LT) {
        this.pos++;
        const third = this.pos < this.srcEnd ? src.charCodeAt(this.pos) : 0;
        if (third === CH_LT) {
          this.pos++;
          this.skipSpacesAndTabs();
          const targetPos = this.pos;
          if (this.pos < this.srcEnd && src.charCodeAt(this.pos) !== CH_NL && src.charCodeAt(this.pos) !== CH_HASH) {
            this.readRedirectTargetText();
          }
          this.redirectToken(out, "<<<", tokenStart, targetPos);
          return true;
        }
        const dash = third === CH_DASH;
        if (dash)
          this.pos++;
        this.skipSpacesAndTabs();
        const targetPos = this.pos;
        if (this.pos >= this.srcEnd || src.charCodeAt(this.pos) !== CH_HASH)
          this.readHereDocDelimiter();
        const hasTarget = this.pos > targetPos;
        if (hasTarget) {
          (this.pendingHereDocs ??= []).push({ delimiter: this._hereDelim, strip: dash, quoted: this._hereQuoted });
        }
        setToken(out, Token.Redirect, dash ? "<<-" : "<<", tokenStart, this.pos);
        out.content = hasTarget ? this._hereDelim : undefined;
        out.targetPos = targetPos;
        out.targetEnd = hasTarget ? this.pos : targetPos;
        return true;
      }
      if (next === CH_LPAREN) {
        this.readProcessSubstitution(out, "<", tokenStart);
        return true;
      }
      if (next === CH_GT) {
        op = "<>";
        this.pos++;
      } else if (next === CH_AMP) {
        op = "<&";
        this.pos++;
      } else {
        op = "<";
      }
    } else if (ch === CH_GT) {
      this.pos++;
      const next = this.pos < this.srcEnd ? src.charCodeAt(this.pos) : 0;
      if (next === CH_LPAREN) {
        this.readProcessSubstitution(out, ">", tokenStart);
        return true;
      }
      if (next === CH_GT) {
        op = ">>";
        this.pos++;
      } else if (next === CH_AMP) {
        op = ">&";
        this.pos++;
      } else if (next === CH_PIPE) {
        op = ">|";
        this.pos++;
      } else {
        op = ">";
      }
    }
    this.skipSpacesAndTabs();
    if (this.pos < this.srcEnd) {
      const nc = src.charCodeAt(this.pos);
      if ((nc === CH_LT || nc === CH_GT) && this.pos + 1 < this.srcEnd && src.charCodeAt(this.pos + 1) === CH_LPAREN) {
        const psStart = this.pos;
        this.pos += 2;
        this.extractBalanced();
        if (this._unbalanced)
          this.errors.push({ message: "unterminated process substitution", pos: psStart });
        const psText = src.slice(psStart, this.pos);
        setToken(out, Token.Redirect, op, tokenStart, this.pos);
        out.content = psText;
        out.targetPos = psStart;
        out.targetEnd = this.pos;
        return true;
      }
      const targetPos = this.pos;
      if (nc !== CH_NL && nc !== CH_HASH)
        this.readRedirectTargetText();
      this.redirectToken(out, op, tokenStart, targetPos);
      return true;
    }
    this.redirectToken(out, op, tokenStart, this.pos);
    return true;
  }
  readRedirectTargetText() {
    const savedBuildValue = this._buildValue;
    this._buildValue = true;
    this.readWordText();
    this._buildValue = savedBuildValue;
  }
  redirectToken(out, operator, tokenStart, targetPos) {
    const hasTarget = this.pos > targetPos && (this._wordText.length > 0 || this._wordQuoted);
    setToken(out, Token.Redirect, operator, tokenStart, this.pos);
    out.content = hasTarget ? this._wordText : undefined;
    out.targetPos = targetPos;
    out.targetEnd = hasTarget ? this.pos : targetPos;
  }
  readProcessSubstitution(out, operator, tokenStart) {
    this.pos++;
    this.extractBalanced();
    if (this._unbalanced)
      this.errors.push({ message: "unterminated process substitution", pos: tokenStart });
    const text = this.src.slice(tokenStart, this.pos);
    setToken(out, Token.Word, text, tokenStart, this.pos);
  }
  readHereDocDelimiter() {
    const src = this.src;
    const len = this.srcEnd;
    const savedBuildValue = this._buildValue;
    this._buildValue = true;
    let delimiter = "";
    let quoted = false;
    while (this.pos < len) {
      const c = src.charCodeAt(this.pos);
      if (c === CH_SQUOTE) {
        quoted = true;
        this.pos++;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_SQUOTE) {
          delimiter += src[this.pos];
          this.pos++;
        }
        if (this.pos < len)
          this.pos++;
      } else if (c === CH_DQUOTE) {
        quoted = true;
        this.pos++;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_DQUOTE) {
          if (src.charCodeAt(this.pos) === CH_BACKSLASH && this.pos + 1 < len) {
            const next = src.charCodeAt(this.pos + 1);
            if (next === CH_NL) {
              this.pos += 2;
              continue;
            }
            if (next === CH_DOLLAR || next === CH_BACKTICK || next === CH_DQUOTE || next === CH_BACKSLASH)
              this.pos++;
          }
          delimiter += src[this.pos];
          this.pos++;
        }
        if (this.pos < len)
          this.pos++;
      } else if (c === CH_BACKSLASH) {
        if (this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_NL) {
          this.pos += 2;
          continue;
        }
        quoted = true;
        this.pos++;
        if (this.pos < len) {
          delimiter += src[this.pos];
          this.pos++;
        } else {
          delimiter += "\\";
        }
      } else if (c === CH_BACKTICK) {
        const btStart = this.pos;
        this.pos++;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
          if (src.charCodeAt(this.pos) === CH_BACKSLASH)
            this.pos++;
          this.pos++;
        }
        if (this.pos < len)
          this.pos++;
        delimiter += src.slice(btStart, this.pos);
      } else if (c === CH_DOLLAR) {
        const next = this.pos + 1 < len ? src.charCodeAt(this.pos + 1) : 0;
        if (next === CH_SQUOTE || next === CH_DQUOTE)
          quoted = true;
        this.readDollar();
        delimiter += this._resultText;
      } else if (c < 128 && charType[c] & 1) {
        break;
      } else {
        delimiter += src[this.pos];
        this.pos++;
      }
    }
    this._buildValue = savedBuildValue;
    this._hereDelim = delimiter;
    this._hereQuoted = quoted;
  }
  consumePendingHereDocs() {
    const pending = this.pendingHereDocs;
    if (pending === null || pending.length === 0)
      return;
    for (const hd of pending) {
      const bodyPos = this.pos;
      const body = this.readHereDocBody(hd.delimiter, hd.strip);
      if (hd.target) {
        hd.target.content = body;
        if (hd.quoted) {
          hd.target.heredocQuoted = true;
        } else if (body) {
          const parsed = this.parseHereDocBody(body, bodyPos);
          if (parsed)
            hd.target.body = parsed;
        }
      }
    }
    pending.length = 0;
  }
  readHereDocBody(delimiter, strip) {
    const bodyStart = this.pos;
    const bodyEnd = this.skipHereDocBody(delimiter, strip);
    return this.src.slice(bodyStart, bodyEnd);
  }
  matchHereDocDelimiter(delimiter, lineStart, end, join) {
    const src = this.src;
    let pos = lineStart;
    for (let i = 0;i < delimiter.length; ) {
      if (join && src.charCodeAt(pos) === CH_BACKSLASH && pos + 1 < end && src.charCodeAt(pos + 1) === CH_NL) {
        pos += 2;
        continue;
      }
      if (pos >= end || src.charCodeAt(pos) !== delimiter.charCodeAt(i))
        return -1;
      pos++;
      i++;
    }
    return pos;
  }
  logicalLineEnd(from, end, join) {
    const src = this.src;
    let pos = from;
    while (pos < end) {
      const c = src.charCodeAt(pos);
      if (c === CH_NL)
        return pos;
      pos += join && c === CH_BACKSLASH ? 2 : 1;
    }
    return end;
  }
  skipHereDocBody(delimiter, strip, parenEnds = false, quoted = false) {
    const src = this.src;
    const len = this.srcEnd;
    const dLen = delimiter.length;
    while (this.pos < len) {
      let lineStart = this.pos;
      let lineEnd = src.indexOf(`
`, this.pos);
      if (lineEnd === -1 || lineEnd > len)
        lineEnd = len;
      if (strip) {
        while (lineStart < lineEnd && src.charCodeAt(lineStart) === CH_TAB)
          lineStart++;
      }
      if (lineEnd - lineStart === dLen && src.startsWith(delimiter, lineStart)) {
        const bodyEnd = this.pos;
        this.pos = lineEnd < len ? lineEnd + 1 : lineEnd;
        return bodyEnd;
      }
      if (parenEnds) {
        const afterDelim = this.matchHereDocDelimiter(delimiter, lineStart, len, !quoted);
        if (afterDelim !== -1) {
          const paren = src.indexOf(")", afterDelim);
          if (paren !== -1 && paren < this.logicalLineEnd(lineStart, len, !quoted)) {
            const bodyEnd = this.pos;
            this.pos = afterDelim;
            return bodyEnd;
          }
        }
      }
      this.pos = lineEnd < len ? lineEnd + 1 : lineEnd;
    }
    return this.pos;
  }
  parseHereDocBody(body, bodyPos) {
    let hasExpansion = false;
    for (let i = 0;i < body.length; i++) {
      const c = body.charCodeAt(i);
      if (c === CH_BACKTICK) {
        hasExpansion = true;
        break;
      }
      if (c === CH_DOLLAR) {
        const next = i + 1 < body.length ? body.charCodeAt(i + 1) : 0;
        if (next === CH_LBRACE || next === CH_LPAREN || next === CH_DOLLAR || next >= CH_a && next <= CH_z || next >= CH_A && next <= CH_Z || next === CH_UNDERSCORE || next === CH_BANG || next === CH_HASH || next === CH_AT || next === CH_STAR || next === CH_QUESTION || next === CH_DASH || next >= CH_0 && next <= CH_9) {
          hasExpansion = true;
          break;
        }
      }
      if (c === CH_BACKSLASH)
        i++;
    }
    if (!hasExpansion)
      return null;
    return new WordImpl(body, bodyPos, bodyPos + body.length, this.src, WordImpl._resolveHeredocBody, this._nestingDepth);
  }
  _wordText = "";
  _wordRaw = false;
  _wordQuoted = false;
  _wordHasExpansions = false;
  _wordKeywordEligible = false;
  _wordIsAssignment;
  _wordAssignmentOperatorPos;
  _wordParts = null;
  _resultText = "";
  _resultIsRaw = true;
  _resultHasExpansion = false;
  _resultPart;
  _unbalanced = false;
  _notArithmetic = false;
  _dqText = "";
  _dqHasExpansions = false;
  _dqParts = null;
  _dqEnd = 0;
  _hereDelim = "";
  _hereQuoted = false;
  readWord(out, ctx, tokenStart = 0) {
    this.readWordText();
    this.classifyWord(out, ctx, tokenStart);
  }
  classifyWord(out, ctx, tokenStart) {
    const src = this.src;
    const raw = this._wordRaw;
    const hasExpansions = this._wordHasExpansions;
    const quoted = this._wordQuoted;
    const keywordEligible = this._wordKeywordEligible;
    const isAssignment = this._wordIsAssignment;
    let assignmentOpPos = this._wordAssignmentOperatorPos;
    const wordEnd = this.pos;
    const wordLen = wordEnd - tokenStart;
    let value = null;
    if (!raw && !hasExpansions) {
      const nextCh = wordEnd < this.srcEnd ? src.charCodeAt(wordEnd) : 0;
      if (!quoted && wordLen <= 16 || nextCh === CH_LT || nextCh === CH_GT) {
        value = this.wordValueOf(tokenStart, wordEnd);
      }
    }
    if (ctx === LexContext.CommandStart && keywordEligible) {
      if (raw) {
        if (wordLen <= 8) {
          const reserved = matchReservedWord(src, tokenStart, wordLen);
          if (reserved !== undefined) {
            setSpanToken(out, reserved, tokenStart, wordEnd, true);
            return;
          }
        }
        if (wordLen === 2 && src.charCodeAt(tokenStart) === CH_LBRACKET && src.charCodeAt(tokenStart + 1) === CH_LBRACKET) {
          setSpanToken(out, Token.DblLBracket, tokenStart, wordEnd, true);
          return;
        }
      } else if (value !== null && value.length > 0) {
        const fc = value.charCodeAt(0);
        if (fc >= CH_a && fc <= CH_z && value.length <= 8 || fc === CH_BANG || fc === CH_LBRACE || fc === CH_RBRACE) {
          const reserved = RESERVED_WORDS.get(value);
          if (reserved !== undefined) {
            setToken(out, reserved, value, tokenStart, wordEnd);
            return;
          }
        }
        if (fc === CH_LBRACKET && value === "[[") {
          setToken(out, Token.DblLBracket, value, tokenStart, wordEnd);
          return;
        }
      }
    }
    if (ctx === LexContext.CommandStart || ctx === LexContext.CommandPrefix) {
      if (isAssignment === undefined) {
        let eq = -1;
        let bracket = false;
        for (let i = tokenStart + 1;i < wordEnd; i++) {
          const c = src.charCodeAt(i);
          if (c === CH_EQ) {
            eq = i;
            break;
          }
          if (c === CH_LBRACKET)
            bracket = true;
        }
        if (eq !== -1) {
          const state = scanAssignmentPrefix(src, tokenStart, wordEnd, ASSIGNMENT_NAME_START);
          if (isMatchedAssignment(state))
            assignmentOpPos = assignmentOperatorPos(state);
        } else if (bracket && wordEnd < this.srcEnd && scanAssignmentPrefix(src, tokenStart, wordEnd, ASSIGNMENT_NAME_START) >= ASSIGNMENT_INDEX_BASE) {
          this.pos = tokenStart;
          this.readWordText(true);
          this.classifyWord(out, ctx, tokenStart);
          return;
        }
      }
      if (assignmentOpPos !== undefined) {
        setSpanToken(out, Token.Assignment, tokenStart, wordEnd, raw);
        if (value !== null)
          out._value = value;
        out.assignmentOperatorPos = assignmentOpPos;
        return;
      }
    }
    if ((ctx === LexContext.CommandStart || ctx === LexContext.TestMode) && keywordEligible) {
      if (raw) {
        if (wordLen === 2 && src.charCodeAt(tokenStart) === CH_RBRACKET && src.charCodeAt(tokenStart + 1) === CH_RBRACKET) {
          setSpanToken(out, Token.DblRBracket, tokenStart, wordEnd, true);
          return;
        }
      } else if (value === "]]") {
        setToken(out, Token.DblRBracket, value, tokenStart, wordEnd);
        return;
      }
    }
    if (!hasExpansions && this.pos < this.srcEnd) {
      const nc = src.charCodeAt(this.pos);
      if (nc === CH_LT || nc === CH_GT) {
        if (raw) {
          const fc = src.charCodeAt(tokenStart);
          if (fc >= CH_0 && fc <= CH_9 && isAllDigitsRange(src, tokenStart, wordEnd)) {
            const fd = Number.parseInt(src.slice(tokenStart, wordEnd), 10);
            if (this.readRedirection(out, tokenStart)) {
              out.fileDescriptor = fd;
              return;
            }
          }
          if (fc === CH_LBRACE && wordLen > 2 && src.charCodeAt(wordEnd - 1) === CH_RBRACE) {
            const varname = src.slice(tokenStart + 1, wordEnd - 1);
            if (this.readRedirection(out, tokenStart)) {
              out.variableName = varname;
              return;
            }
          }
        } else if (value !== null && value.length > 0) {
          if (value.charCodeAt(0) >= CH_0 && value.charCodeAt(0) <= CH_9 && isAllDigits(value)) {
            const fd = Number.parseInt(value, 10);
            if (this.readRedirection(out, tokenStart)) {
              out.fileDescriptor = fd;
              return;
            }
          }
          if (value.charCodeAt(0) === CH_LBRACE && value.charCodeAt(value.length - 1) === CH_RBRACE && value.length > 2) {
            const varname = value.slice(1, -1);
            if (this.readRedirection(out, tokenStart)) {
              out.variableName = varname;
              return;
            }
          }
        }
      }
    }
    setSpanToken(out, Token.Word, tokenStart, wordEnd, raw);
    if (value !== null)
      out._value = value;
    out.keywordEligible = keywordEligible;
  }
  readWordText(subscripts = false) {
    const src = this.src;
    const len = this.srcEnd;
    let pos = this.pos;
    const fastStart = pos;
    let exitCh = 0;
    while (pos < len) {
      const c = src.charCodeAt(pos);
      if (c < 128 && charType[c]) {
        exitCh = c;
        break;
      }
      pos++;
    }
    if (pos >= len || charType[exitCh] & 1 && !(exitCh === CH_LPAREN && pos > fastStart && extglobPrefix[src.charCodeAt(pos - 1)]) && !subscripts) {
      this.pos = pos;
      this._wordText = (this._buildParts || this._buildValue) && pos > fastStart ? src.slice(fastStart, pos) : "";
      this._wordRaw = true;
      this._wordQuoted = false;
      this._wordHasExpansions = false;
      this._wordKeywordEligible = true;
      this._wordIsAssignment = undefined;
      this._wordAssignmentOperatorPos = undefined;
      if (this._buildParts)
        this._wordParts = null;
      return;
    }
    const bp = this._buildParts;
    const bt = bp || this._buildValue;
    let text = bt && pos > fastStart ? src.slice(fastStart, pos) : "";
    let quoted = false;
    let hasExpansions = false;
    let keywordEligible = true;
    let valueIsRaw = true;
    let lastValueChar = pos > fastStart ? src.charCodeAt(pos - 1) : 0;
    let assignmentState = scanAssignmentPrefix(src, fastStart, pos, ASSIGNMENT_NAME_START);
    let parts;
    let litBuf = "";
    let litStart = 0;
    if (bp) {
      parts = [];
      litBuf = text;
      litStart = fastStart;
    }
    while (pos < len) {
      const ch = src.charCodeAt(pos);
      if (ch >= 128 || !charType[ch]) {
        const runStart = pos;
        pos++;
        while (pos < len) {
          const c = src.charCodeAt(pos);
          if (c < 128 && charType[c])
            break;
          pos++;
        }
        lastValueChar = src.charCodeAt(pos - 1);
        assignmentState = scanAssignmentPrefix(src, runStart, pos, assignmentState);
        if (bt) {
          const chunk = src.slice(runStart, pos);
          text += chunk;
          if (bp)
            litBuf += chunk;
        }
        continue;
      }
      if (charType[ch] & 1) {
        if (ch === CH_LPAREN && lastValueChar < 128 && extglobPrefix[lastValueChar]) {
          keywordEligible = false;
          const prefixChar = lastValueChar;
          pos++;
          const innerStart = pos;
          const close = this.findClosingShellDelimiter(innerStart, len, CH_RPAREN, prefixChar === CH_EQ);
          const patternEnd = close === -1 ? len : close;
          pos = close === -1 ? len : close + 1;
          if (close === -1)
            this.errors.push({ message: "unterminated extended glob", pos: innerStart - 2 });
          lastValueChar = src.charCodeAt(pos - 1);
          if (bt) {
            const eg = "(" + src.slice(innerStart, pos);
            text += eg;
            if (bp && prefixChar !== CH_EQ) {
              if (litBuf.length > 0) {
                const trimmed = litBuf.slice(0, -1);
                if (trimmed)
                  parts.push({ type: "Literal", value: trimmed, text: src.slice(litStart, innerStart - 2) });
                litBuf = "";
              }
              const op = extglobOp[prefixChar];
              parts.push({
                type: "ExtendedGlob",
                text: op + eg,
                operator: op,
                pattern: src.slice(innerStart, patternEnd),
                parts: hasEmbeddedWordStructure(src, innerStart, patternEnd) ? this.parseSubFieldWord(innerStart, patternEnd).parts : undefined
              });
              litStart = pos;
            } else if (bp) {
              litBuf += eg;
            }
          }
          continue;
        }
        if (subscripts && assignmentState >= ASSIGNMENT_INDEX_BASE) {
          const close = this.findClosingBracket(pos);
          if (close !== -1) {
            const spanEnd = close + 1;
            assignmentState = scanAssignmentPrefix(src, pos, spanEnd, assignmentState);
            lastValueChar = src.charCodeAt(close);
            if (bt) {
              const chunk = src.slice(pos, spanEnd);
              text += chunk;
              if (bp)
                litBuf += chunk;
            }
            pos = spanEnd;
            continue;
          }
        }
        break;
      }
      if (ch === CH_BACKSLASH) {
        pos++;
        if (pos < len) {
          if (src.charCodeAt(pos) === CH_NL) {
            pos++;
            valueIsRaw = false;
          } else {
            if (assignmentState >= 0 && assignmentState < ASSIGNMENT_INDEX_BASE)
              assignmentState = ASSIGNMENT_INVALID;
            quoted = true;
            keywordEligible = false;
            valueIsRaw = false;
            lastValueChar = src.charCodeAt(pos);
            if (bt) {
              text += src[pos];
              if (bp)
                litBuf += src[pos];
            }
            pos++;
          }
        } else {
          if (assignmentState >= 0 && assignmentState < ASSIGNMENT_INDEX_BASE)
            assignmentState = ASSIGNMENT_INVALID;
          quoted = true;
          keywordEligible = false;
          lastValueChar = CH_BACKSLASH;
          if (bt) {
            text += "\\";
            if (bp)
              litBuf += "\\";
          }
        }
        continue;
      }
      if (ch === CH_SQUOTE) {
        const sqStart = pos;
        if (assignmentState >= 0 && assignmentState < ASSIGNMENT_INDEX_BASE)
          assignmentState = ASSIGNMENT_INVALID;
        quoted = true;
        keywordEligible = false;
        valueIsRaw = false;
        pos++;
        const start = pos;
        while (pos < len && src.charCodeAt(pos) !== CH_SQUOTE)
          pos++;
        if (pos > start)
          lastValueChar = src.charCodeAt(pos - 1);
        const value = bt ? src.slice(start, pos) : "";
        if (bt)
          text += value;
        if (pos < len)
          pos++;
        else
          this.errors.push({ message: "unterminated single quote", pos: start - 1 });
        if (bp) {
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, sqStart) });
            litBuf = "";
          }
          parts.push({ type: "SingleQuoted", value, text: src.slice(sqStart, pos) });
          litStart = pos;
        }
        continue;
      }
      if (ch === CH_DQUOTE) {
        const dqStart = pos;
        if (assignmentState >= 0 && assignmentState < ASSIGNMENT_INDEX_BASE)
          assignmentState = ASSIGNMENT_INVALID;
        quoted = true;
        keywordEligible = false;
        valueIsRaw = false;
        pos++;
        this.pos = pos;
        this.readDoubleQuoted();
        pos = this.pos;
        if (this._dqEnd > dqStart + 1)
          lastValueChar = src.charCodeAt(this._dqEnd - 1);
        if (this._dqHasExpansions)
          hasExpansions = true;
        if (bt)
          text += this._dqText;
        if (bp) {
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, dqStart) });
            litBuf = "";
          }
          const dqText = src.slice(dqStart, pos);
          parts.push({
            type: "DoubleQuoted",
            text: dqText,
            parts: this._dqParts ?? [
              { type: "Literal", value: this._dqText, text: src.slice(dqStart + 1, this._dqEnd) }
            ]
          });
          litStart = pos;
        }
        continue;
      }
      if (ch === CH_DOLLAR) {
        keywordEligible = false;
        const dollarStart = pos;
        if (assignmentState >= 0 && assignmentState < ASSIGNMENT_INDEX_BASE)
          assignmentState = ASSIGNMENT_INVALID;
        this.pos = pos;
        this.readDollar();
        pos = this.pos;
        if (!this._resultIsRaw)
          valueIsRaw = false;
        if (pos > dollarStart)
          lastValueChar = src.charCodeAt(pos - 1);
        if (this._resultHasExpansion)
          hasExpansions = true;
        if (bt)
          text += this._resultText;
        if (bp) {
          if (this._resultPart) {
            if (litBuf) {
              parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, dollarStart) });
              litBuf = "";
            }
            parts.push(this._resultPart);
            litStart = pos;
          } else {
            litBuf += this._resultText;
          }
        }
        continue;
      }
      if (ch === CH_BACKTICK) {
        keywordEligible = false;
        const btStart = pos;
        if (assignmentState >= 0 && assignmentState < ASSIGNMENT_INDEX_BASE)
          assignmentState = ASSIGNMENT_INVALID;
        this.pos = pos;
        this.readBacktickExpansion();
        pos = this.pos;
        valueIsRaw = false;
        lastValueChar = src.charCodeAt(pos - 1);
        hasExpansions = true;
        if (bt)
          text += this._resultText;
        if (bp) {
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, btStart) });
            litBuf = "";
          }
          parts.push(this._resultPart);
          litStart = pos;
        }
        continue;
      }
      if (ch === CH_LBRACE) {
        if (assignmentState >= 0 && assignmentState < ASSIGNMENT_INDEX_BASE)
          assignmentState = ASSIGNMENT_INVALID;
        const braceEnd = scanBraceExpansion(src, pos, len);
        if (braceEnd > 0) {
          keywordEligible = false;
          lastValueChar = src.charCodeAt(braceEnd - 1);
          if (bt) {
            const braceText = src.slice(pos, braceEnd);
            text += braceText;
            if (bp) {
              if (litBuf) {
                parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, pos) });
                litBuf = "";
              }
              parts.push({
                type: "BraceExpansion",
                text: braceText,
                parts: hasEmbeddedWordStructure(src, pos + 1, braceEnd - 1) ? this.parseSubFieldWord(pos + 1, braceEnd - 1).parts : undefined
              });
              litStart = braceEnd;
            }
          }
          pos = braceEnd;
          continue;
        }
        lastValueChar = CH_LBRACE;
        if (bt) {
          text += "{";
          if (bp)
            litBuf += "{";
        }
        pos++;
        continue;
      }
      pos++;
    }
    if (bp && litBuf)
      parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, pos) });
    this.pos = pos;
    this._wordText = text;
    this._wordRaw = valueIsRaw;
    this._wordQuoted = quoted;
    this._wordHasExpansions = hasExpansions;
    this._wordKeywordEligible = keywordEligible;
    this._wordIsAssignment = isMatchedAssignment(assignmentState);
    this._wordAssignmentOperatorPos = this._wordIsAssignment ? assignmentOperatorPos(assignmentState) : undefined;
    if (bp) {
      this._wordParts = parts.length > 1 || parts.length === 1 && parts[0].type !== "Literal" ? parts : null;
    }
  }
  readInnerWordText() {
    const src = this.src;
    const len = this.srcEnd;
    let pos = this.pos;
    let text = "";
    const bp = this._buildParts;
    let parts;
    let litBuf = "";
    let litStart = 0;
    if (bp) {
      parts = [];
      litStart = pos;
    }
    while (pos < len) {
      const ch = src.charCodeAt(pos);
      if (ch === CH_BACKSLASH) {
        pos++;
        if (pos < len) {
          if (src.charCodeAt(pos) === CH_NL) {
            pos++;
          } else {
            const escaped = src[pos++];
            text += escaped;
            if (bp)
              litBuf += escaped;
          }
        }
        continue;
      }
      if (ch === CH_SQUOTE) {
        const sqStart = pos;
        pos++;
        const start = pos;
        while (pos < len && src.charCodeAt(pos) !== CH_SQUOTE)
          pos++;
        const value = src.slice(start, pos);
        text += value;
        if (pos < len)
          pos++;
        if (bp) {
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, sqStart) });
            litBuf = "";
          }
          parts.push({ type: "SingleQuoted", value, text: src.slice(sqStart, pos) });
          litStart = pos;
        }
        continue;
      }
      if (ch === CH_DQUOTE) {
        const dqStart = pos;
        pos++;
        this.pos = pos;
        this.readDoubleQuoted();
        pos = this.pos;
        text += this._dqText;
        if (bp) {
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, dqStart) });
            litBuf = "";
          }
          const dqText = src.slice(dqStart, pos);
          parts.push({
            type: "DoubleQuoted",
            text: dqText,
            parts: this._dqParts ?? [
              { type: "Literal", value: this._dqText, text: src.slice(dqStart + 1, this._dqEnd) }
            ]
          });
          litStart = pos;
        }
        continue;
      }
      if (ch === CH_DOLLAR) {
        const dollarStart = pos;
        this.pos = pos;
        this.readDollar();
        pos = this.pos;
        text += this._resultText;
        if (bp) {
          if (this._resultPart) {
            if (litBuf) {
              parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, dollarStart) });
              litBuf = "";
            }
            parts.push(this._resultPart);
            litStart = pos;
          } else {
            litBuf += this._resultText;
          }
        }
        continue;
      }
      if (ch === CH_BACKTICK) {
        const btStart = pos;
        this.pos = pos;
        this.readBacktickExpansion();
        pos = this.pos;
        text += this._resultText;
        if (bp) {
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, btStart) });
            litBuf = "";
          }
          parts.push(this._resultPart);
          litStart = pos;
        }
        continue;
      }
      if ((ch === CH_LT || ch === CH_GT) && pos + 1 < len && src.charCodeAt(pos + 1) === CH_LPAREN) {
        const psStart = pos;
        this.pos = pos + 2;
        const inner = this.extractBalanced();
        pos = this.pos;
        const raw = src.slice(psStart, pos);
        text += raw;
        if (bp) {
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, psStart) });
            litBuf = "";
          }
          const part = {
            type: "ProcessSubstitution",
            text: raw,
            operator: ch === CH_LT ? "<" : ">",
            script: undefined,
            inner,
            innerStart: psStart + 2
          };
          parts.push(part);
          this.collect(part);
          litStart = pos;
        }
        continue;
      }
      text += src[pos];
      if (bp)
        litBuf += src[pos];
      pos++;
    }
    if (bp && litBuf)
      parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, pos) });
    this.pos = pos;
    this._wordText = text;
    this._wordRaw = false;
    this._wordQuoted = false;
    this._wordHasExpansions = false;
    this._wordKeywordEligible = false;
    if (bp) {
      this._wordParts = parts.length > 1 || parts.length === 1 && parts[0].type !== "Literal" ? parts : null;
    }
  }
  parseSubFieldWord(start, end) {
    if (start >= end)
      return new WordImpl("", start, start);
    if (this._nestingDepth >= MAX_SYNTAX_NESTING)
      return new WordImpl(this.src.slice(start, end), start, end);
    this._nestingDepth++;
    const savedEnd = this.srcEnd;
    const savedPos = this.pos;
    const savedText = this._wordText;
    const savedParts = this._wordParts;
    const savedQuoted = this._wordQuoted;
    const savedKeywordEligible = this._wordKeywordEligible;
    this.srcEnd = end;
    this.pos = start;
    this.readInnerWordText();
    const word = new WordImpl(this.src.slice(start, end), start, end);
    if (this._buildParts && this._wordParts) {
      word.parts = this._wordParts;
    }
    this.srcEnd = savedEnd;
    this.pos = savedPos;
    this._wordText = savedText;
    this._wordParts = savedParts;
    this._wordQuoted = savedQuoted;
    this._wordKeywordEligible = savedKeywordEligible;
    this._nestingDepth--;
    return word;
  }
  skipSQ() {
    while (this.pos < this.srcEnd && this.src.charCodeAt(this.pos) !== CH_SQUOTE)
      this.pos++;
    if (this.pos < this.srcEnd)
      this.pos++;
  }
  skipAnsiCQuoted() {
    const quotePos = this.pos - 1;
    const result = decodeAnsiCQuoted(this.src, this.pos, this.srcEnd);
    this.pos = result.end;
    if (!result.closed)
      this.errors.push({ message: "unterminated ANSI-C quote", pos: quotePos });
  }
  skipDQ() {
    const src = this.src;
    const len = this.srcEnd;
    while (this.pos < len) {
      const ch = src.charCodeAt(this.pos);
      if (ch === CH_DQUOTE) {
        this.pos++;
        return;
      }
      if (ch === CH_BACKSLASH) {
        this.pos += 2;
        continue;
      }
      if (ch === CH_DOLLAR && this.pos + 1 < len) {
        const next = src.charCodeAt(this.pos + 1);
        if (next === CH_LPAREN) {
          const csStart = this.pos;
          this.pos += 2;
          this.extractBalanced();
          if (this._unbalanced)
            this.errors.push({ message: "unterminated command substitution", pos: csStart });
          continue;
        }
        if (next === CH_LBRACE) {
          this.pos += 2;
          let d = 1;
          while (this.pos < len && d > 0) {
            const c = src.charCodeAt(this.pos);
            if (c === CH_RBRACE) {
              if (--d === 0) {
                this.pos++;
                break;
              }
            } else if (c === CH_LBRACE && this.pos > 0 && src.charCodeAt(this.pos - 1) === CH_DOLLAR)
              d++;
            else if (c === CH_BACKSLASH) {
              this.pos++;
            } else if (c === CH_SQUOTE) {
              this.pos++;
              this.skipSQ();
              continue;
            } else if (c === CH_DQUOTE) {
              this.pos++;
              this.skipDQ();
              continue;
            }
            this.pos++;
          }
          continue;
        }
      }
      if (ch === CH_BACKTICK) {
        this.pos++;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
          if (src.charCodeAt(this.pos) === CH_BACKSLASH)
            this.pos++;
          this.pos++;
        }
        if (this.pos < len)
          this.pos++;
        continue;
      }
      this.pos++;
    }
  }
  skipSpacesAndTabs() {
    const src = this.src;
    const len = this.srcEnd;
    while (this.pos < len) {
      const ch = src.charCodeAt(this.pos);
      if (ch === CH_SPACE || ch === CH_TAB)
        this.pos++;
      else if (ch === CH_BACKSLASH && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_NL)
        this.pos += 2;
      else
        break;
    }
  }
  readDoubleQuoted() {
    const src = this.src;
    const len = this.srcEnd;
    const contentStart = this.pos;
    let hasExpansions = false;
    const bp = this._buildParts;
    const bt = bp || this._buildValue;
    if (!bp) {
      let p = this.pos;
      while (p < len) {
        const c = src.charCodeAt(p);
        if (c === CH_DQUOTE) {
          this._dqText = bt ? src.slice(contentStart, p) : "";
          this._dqEnd = p;
          this.pos = p + 1;
          this._dqHasExpansions = false;
          this._dqParts = null;
          return;
        }
        if (c === CH_DOLLAR || c === CH_BACKTICK || c === CH_BACKSLASH)
          break;
        p++;
      }
    }
    let text = "";
    let parts = null;
    let litBuf = "";
    let litStart = bp ? this.pos : 0;
    while (this.pos < len && src.charCodeAt(this.pos) !== CH_DQUOTE) {
      const runStart = this.pos;
      while (this.pos < len) {
        const c = src.charCodeAt(this.pos);
        if (c === CH_DQUOTE || c === CH_BACKSLASH || c === CH_DOLLAR || c === CH_BACKTICK)
          break;
        this.pos++;
      }
      if (bt && this.pos > runStart) {
        const chunk = src.slice(runStart, this.pos);
        text += chunk;
        if (bp)
          litBuf += chunk;
      }
      if (this.pos >= len || src.charCodeAt(this.pos) === CH_DQUOTE)
        break;
      const ch = src.charCodeAt(this.pos);
      if (ch === CH_BACKSLASH) {
        this.pos++;
        if (this.pos < len) {
          const next = src.charCodeAt(this.pos);
          if (next === CH_NL) {
            this.pos++;
            continue;
          }
          if (bt) {
            if (next === CH_DOLLAR || next === CH_BACKTICK || next === CH_DQUOTE || next === CH_BACKSLASH) {
              const c = src[this.pos];
              text += c;
              if (bp)
                litBuf += c;
            } else {
              const pair = "\\" + src[this.pos];
              text += pair;
              if (bp)
                litBuf += pair;
            }
          }
          this.pos++;
        }
        continue;
      }
      if (ch === CH_DOLLAR) {
        const afterDollar = this.pos + 1 < len ? src.charCodeAt(this.pos + 1) : 0;
        if (afterDollar === CH_DQUOTE || afterDollar === CH_SQUOTE) {
          if (bt) {
            text += "$";
            if (bp)
              litBuf += "$";
          }
          this.pos++;
          continue;
        }
        const expStart = this.pos;
        this.readDollar();
        if (bt)
          text += this._resultText;
        if (this._resultHasExpansion)
          hasExpansions = true;
        if (bp) {
          const rp = this._resultPart;
          if (rp && isDQChild(rp)) {
            if (!parts)
              parts = [];
            if (litBuf) {
              parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, expStart) });
              litBuf = "";
            }
            parts.push(rp);
            litStart = this.pos;
          } else {
            litBuf += this._resultText;
          }
        }
        continue;
      }
      if (ch === CH_BACKTICK) {
        const btStart = this.pos;
        this.readBacktickExpansion(true);
        if (bt)
          text += this._resultText;
        hasExpansions = true;
        if (bp && this._resultPart && isDQChild(this._resultPart)) {
          if (!parts)
            parts = [];
          if (litBuf) {
            parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, btStart) });
            litBuf = "";
          }
          parts.push(this._resultPart);
          litStart = this.pos;
        }
        continue;
      }
    }
    if (bp && parts && litBuf)
      parts.push({ type: "Literal", value: litBuf, text: src.slice(litStart, this.pos) });
    this._dqEnd = this.pos;
    if (this.pos < len)
      this.pos++;
    else
      this.errors.push({ message: "unterminated double quote", pos: contentStart - 1 });
    this._dqText = text;
    this._dqHasExpansions = hasExpansions;
    this._dqParts = parts;
  }
  readDollar() {
    const dollarPos = this.pos;
    this.pos++;
    const src = this.src;
    const len = this.srcEnd;
    const bt = this._buildParts || this._buildValue;
    if (this.pos >= len) {
      this._resultText = "$";
      this._resultIsRaw = true;
      this._resultHasExpansion = false;
      this._resultPart = undefined;
      return;
    }
    const logicalPos = skipLineContinuations(src, this.pos, len);
    if (logicalPos > this.pos && src.charCodeAt(logicalPos) === CH_LPAREN && src.charCodeAt(logicalPos + 1) !== CH_LPAREN)
      this.pos = logicalPos;
    const ch = src.charCodeAt(this.pos);
    if (ch === CH_LPAREN) {
      if (this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_LPAREN) {
        const savedPos = this.pos;
        const savedErrors = this.errors.length;
        this.readArithmeticExpansion();
        if (!this._notArithmetic)
          return;
        this.errors.length = savedErrors;
        this.pos = savedPos;
      }
      this.readCommandSubstitution(dollarPos);
      return;
    }
    if (ch === CH_LBRACE) {
      const after = this.pos + 1 < len ? src.charCodeAt(this.pos + 1) : 0;
      if (after === CH_SPACE || after === CH_TAB || after === CH_NL) {
        this.readBraceCommandSubstitution();
        return;
      }
      if (after === CH_PIPE) {
        this.readValueSubstitution();
        return;
      }
      this.readParameterExpansion();
      return;
    }
    if (ch === CH_SQUOTE) {
      this.pos++;
      if (bt) {
        const value = this.readAnsiCQuoted();
        this._resultText = value;
        this._resultPart = this._buildParts ? { type: "AnsiCQuoted", text: src.slice(dollarPos, this.pos), value } : undefined;
      } else {
        this.skipAnsiCQuoted();
        this._resultText = "";
        this._resultPart = undefined;
      }
      this._resultIsRaw = false;
      this._resultHasExpansion = false;
      return;
    }
    if (ch === CH_DQUOTE) {
      this.pos++;
      this.readDoubleQuoted();
      this._resultText = this._dqText;
      this._resultIsRaw = false;
      this._resultHasExpansion = this._dqHasExpansions;
      if (this._buildParts) {
        const text = src.slice(dollarPos, this.pos);
        this._resultPart = {
          type: "LocaleString",
          text,
          parts: this._dqParts ?? [
            { type: "Literal", value: this._dqText, text: src.slice(dollarPos + 2, this._dqEnd) }
          ]
        };
      } else {
        this._resultPart = undefined;
      }
      return;
    }
    if (ch === CH_AT || ch === CH_STAR || ch === CH_HASH || ch === CH_QUESTION || ch === CH_DASH || ch === CH_DOLLAR || ch === CH_BANG || ch >= CH_0 && ch <= CH_9) {
      this.pos++;
      const text = bt ? src.slice(this.pos - 2, this.pos) : "";
      this._resultText = text;
      this._resultIsRaw = true;
      this._resultHasExpansion = false;
      this._resultPart = this._buildParts ? { type: "SimpleExpansion", text } : undefined;
      return;
    }
    if (ch < 128 && isIdChar[ch] & 1) {
      const namePos = this.pos - 1;
      while (this.pos < len) {
        const c = src.charCodeAt(this.pos);
        if (c < 128 && isIdChar[c] & 2)
          this.pos++;
        else
          break;
      }
      const text = bt ? src.slice(namePos, this.pos) : "";
      this._resultText = text;
      this._resultIsRaw = true;
      this._resultHasExpansion = false;
      this._resultPart = this._buildParts ? { type: "SimpleExpansion", text } : undefined;
      return;
    }
    if (ch === CH_LBRACKET) {
      const close = this.findClosingArithmeticBracket(this.pos + 1);
      if (close !== -1) {
        const bodyStart = this.pos + 1;
        const body = src.slice(bodyStart, close);
        this.pos = close + 1;
        const text = bt ? src.slice(dollarPos, this.pos) : "";
        this._resultText = text;
        this._resultIsRaw = true;
        this._resultHasExpansion = false;
        this._resultPart = this._buildParts ? { type: "ArithmeticExpansion", text, expression: this.buildArithmeticExpression(body, bodyStart) } : undefined;
        return;
      }
    }
    this._resultText = "$";
    this._resultIsRaw = true;
    this._resultHasExpansion = false;
    this._resultPart = undefined;
  }
  scanArithmeticBody() {
    this._notArithmetic = false;
    this.pos += 2;
    let depth = 1;
    let parenDepth = 0;
    let parentParenDepth = 0;
    let parenDepths;
    let expansions = 0;
    let reported = false;
    const src = this.src;
    const len = this.srcEnd;
    const start = this.pos;
    while (this.pos < len && depth > 0) {
      const c = src.charCodeAt(this.pos);
      if (c === CH_BACKSLASH) {
        this.pos += 2;
      } else if (c === CH_SQUOTE) {
        this.pos++;
        this.skipSQ();
      } else if (c === CH_DQUOTE) {
        this.pos++;
        this.skipDQ();
      } else if (c === CH_BACKTICK) {
        this.pos++;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
          if (src.charCodeAt(this.pos) === CH_BACKSLASH)
            this.pos++;
          this.pos++;
        }
        if (this.pos < len)
          this.pos++;
      } else if (c === CH_DOLLAR && this.pos + 2 < len && src.charCodeAt(this.pos + 1) === CH_LPAREN && src.charCodeAt(this.pos + 2) !== CH_LPAREN) {
        const dollarPos = this.pos;
        this.pos += 2;
        this.extractBalanced();
        if (this._unbalanced)
          this.errors.push({ message: "unterminated command substitution", pos: dollarPos });
      } else if (c === CH_DOLLAR && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_LBRACE) {
        const close = this.findClosingBrace(this.pos + 2, len);
        this.pos = close === -1 ? len : close + 1;
      } else if ((c === CH_LT || c === CH_GT) && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_LPAREN) {
        this.pos += 2;
        this.extractBalanced();
      } else if (c === CH_LPAREN) {
        if (src.charCodeAt(this.pos - 1) === CH_DOLLAR && src.charCodeAt(this.pos + 1) === CH_LPAREN) {
          if (depth === 1)
            parentParenDepth = parenDepth;
          else
            (parenDepths ??= []).push(parenDepth);
          depth++;
          parenDepth = 0;
          if (++expansions + this._nestingDepth >= MAX_SYNTAX_NESTING) {
            if (!reported) {
              this.errors.push({ message: "maximum arithmetic expansion nesting depth exceeded", pos: this.pos - 1 });
              reported = true;
            }
          }
          this.pos += 2;
        } else {
          parenDepth++;
          this.pos++;
        }
      } else if (c === CH_RPAREN && parenDepth > 0) {
        parenDepth--;
        this.pos++;
      } else if (c === CH_RPAREN && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_RPAREN) {
        if (--depth === 0) {
          this.pos += 2;
          break;
        }
        parenDepth = depth === 1 ? parentParenDepth : parenDepths.pop();
        this.pos += 2;
      } else if (c === CH_RPAREN && depth === 1) {
        this._notArithmetic = true;
        return "";
      } else {
        this.pos++;
      }
    }
    return this._buildParts || this._buildValue ? src.slice(start, this.pos - 2) : "";
  }
  readArithmeticExpansion() {
    const bodyStart = this.pos + 2;
    const body = this.scanArithmeticBody();
    if (this._notArithmetic)
      return;
    const text = this._buildParts || this._buildValue ? "$((" + body + "))" : "";
    this._resultText = text;
    this._resultIsRaw = true;
    this._resultHasExpansion = false;
    this._resultPart = this._buildParts ? { type: "ArithmeticExpansion", text, expression: this.buildArithmeticExpression(body, bodyStart) } : undefined;
  }
  buildArithmeticExpression(body, bodyStart) {
    if (!hasEmbeddedWordStructure(this.src, bodyStart, bodyStart + body.length)) {
      return parseArithmeticExpression(body, bodyStart) ?? undefined;
    }
    const commandExpansions = [];
    const embeddedWords = [];
    const expr = parseArithmeticExpression(body, bodyStart, {
      commandExpansions,
      embeddedWords,
      findClosingBracket: (start, end) => this.findClosingBracket(start, end),
      findClosingBrace: (start, end) => this.findClosingBrace(start, end),
      findClosingParenthesis: (start, end) => this.findClosingParenthesis(start, end),
      findArithmeticExpansionEnd: (start, end) => this.findArithmeticExpansionEnd(start, end),
      findArithmeticWordEnd: (start, end) => this.findArithmeticWordEnd(start, end)
    }) ?? undefined;
    for (const node of commandExpansions) {
      node.innerStart = node.pos + 2;
      this.collect(node);
    }
    for (const node of embeddedWords)
      node.parts = this.parseSubFieldWord(node.pos, node.end).parts;
    return expr;
  }
  readArithmeticCommand(out, tokenStart) {
    const savedBuildValue = this._buildValue;
    this._buildValue = true;
    const body = this.scanArithmeticBody();
    this._buildValue = savedBuildValue;
    setToken(out, Token.ArithCmd, body, tokenStart, this.pos);
  }
  readCommandSubstitution(dollarPos) {
    const openPos = this.pos;
    this.pos++;
    const inner = this.extractBalanced();
    if (this._unbalanced)
      this.errors.push({ message: "unterminated command substitution", pos: dollarPos });
    const bt = this._buildParts || this._buildValue;
    const rawText = bt ? this.src.slice(dollarPos, this.pos) : "";
    const text = !bt || openPos === dollarPos + 1 ? rawText : "$" + this.src.slice(openPos, this.pos);
    this._resultText = text;
    this._resultIsRaw = openPos === dollarPos + 1;
    this._resultHasExpansion = true;
    if (this._buildParts) {
      this._resultPart = { type: "CommandExpansion", text: rawText, script: undefined, inner, innerStart: openPos + 1 };
      this.collect(this._resultPart);
    } else {
      this._resultPart = undefined;
    }
  }
  readBraceCommandSubstitution() {
    this.readBraceSubstitution(1);
  }
  readValueSubstitution() {
    this.readBraceSubstitution(2);
  }
  readBraceSubstitution(skip) {
    const dollarPos = this.pos - 1;
    this.pos += skip;
    const src = this.src;
    const len = this.srcEnd;
    let depth = 1;
    const start = this.pos;
    while (this.pos < len) {
      const c = src.charCodeAt(this.pos);
      if (c === CH_LBRACE)
        depth++;
      else if (c === CH_RBRACE) {
        if (--depth === 0) {
          this.pos++;
          break;
        }
      } else if (c === CH_SQUOTE) {
        this.pos++;
        this.skipSQ();
        continue;
      } else if (c === CH_DQUOTE) {
        this.pos++;
        this.skipDQ();
        continue;
      } else if (c === CH_BACKSLASH)
        this.pos++;
      this.pos++;
    }
    this._resultIsRaw = true;
    this._resultHasExpansion = true;
    if (this._buildParts || this._buildValue) {
      const rawInner = src.slice(start, this.pos - 1);
      const inner = rawInner.trim();
      const text = src.slice(dollarPos, this.pos);
      this._resultText = text;
      if (this._buildParts) {
        const innerStart = start + (rawInner.length - rawInner.trimStart().length);
        this._resultPart = { type: "CommandExpansion", text, script: undefined, inner, innerStart };
        this.collect(this._resultPart);
      } else {
        this._resultPart = undefined;
      }
    } else {
      this._resultText = "";
      this._resultPart = undefined;
    }
  }
  readBacktickExpansion(insideDoubleQuotes = false) {
    this.pos++;
    const src = this.src;
    const len = this.srcEnd;
    const start = this.pos;
    if (!this._buildParts && !this._buildValue) {
      while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
        if (src.charCodeAt(this.pos) === CH_BACKSLASH && this.pos + 1 < len)
          this.pos++;
        this.pos++;
      }
      if (this.pos < len)
        this.pos++;
      else
        this.errors.push({ message: "unterminated backtick", pos: start - 1 });
      this._resultText = "";
      this._resultIsRaw = false;
      this._resultHasExpansion = true;
      this._resultPart = undefined;
      return;
    }
    let inner = "";
    let hasEscapes = false;
    while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
      if (src.charCodeAt(this.pos) === CH_BACKSLASH) {
        hasEscapes = true;
        break;
      }
      this.pos++;
    }
    if (!hasEscapes) {
      inner = src.slice(start, this.pos);
    } else {
      inner = src.slice(start, this.pos);
      while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
        if (src.charCodeAt(this.pos) === CH_BACKSLASH) {
          this.pos++;
          if (this.pos < len) {
            const c = src.charCodeAt(this.pos);
            if (c === CH_DOLLAR || c === CH_BACKTICK || c === CH_BACKSLASH || insideDoubleQuotes && c === CH_DQUOTE) {
              inner += src[this.pos];
            } else {
              inner += "\\" + src[this.pos];
            }
            this.pos++;
          }
        } else {
          const runStart = this.pos;
          while (this.pos < len) {
            const c = src.charCodeAt(this.pos);
            if (c === CH_BACKTICK || c === CH_BACKSLASH)
              break;
            this.pos++;
          }
          inner += src.slice(runStart, this.pos);
        }
      }
    }
    if (this.pos < len)
      this.pos++;
    else
      this.errors.push({ message: "unterminated backtick", pos: start - 1 });
    const text = src.slice(start - 1, this.pos);
    this._resultText = inner;
    this._resultHasExpansion = true;
    if (this._buildParts) {
      this._resultPart = {
        type: "CommandExpansion",
        text,
        script: undefined,
        inner,
        innerStart: hasEscapes ? undefined : start
      };
      this.collect(this._resultPart);
    } else {
      this._resultPart = undefined;
    }
  }
  readParameterExpansion() {
    const src = this.src;
    const len = this.srcEnd;
    const start = this.pos;
    this.pos++;
    let depth = 1;
    let reported = false;
    while (this.pos < len && depth > 0) {
      const ch = src.charCodeAt(this.pos);
      if (ch === CH_DOLLAR) {
        const next = this.pos + 1 < len ? src.charCodeAt(this.pos + 1) : 0;
        if (next === CH_LBRACE) {
          depth++;
          if (this._nestingDepth + depth > MAX_SYNTAX_NESTING && !reported) {
            this.errors.push({ message: "maximum parameter expansion nesting depth exceeded", pos: this.pos });
            reported = true;
          }
          this.pos += 2;
          continue;
        }
        if (next === CH_DOLLAR) {
          this.pos += 2;
          continue;
        }
        if (next === CH_LPAREN) {
          const dollarPos = this.pos;
          this.pos += 2;
          this.extractBalanced();
          if (this._unbalanced)
            this.errors.push({ message: "unterminated command substitution", pos: dollarPos });
          continue;
        }
      } else if (ch === CH_BACKTICK) {
        this.pos++;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
          if (src.charCodeAt(this.pos) === CH_BACKSLASH)
            this.pos++;
          this.pos++;
        }
        if (this.pos < len)
          this.pos++;
        continue;
      } else if (ch === CH_RBRACE) {
        if (--depth === 0) {
          this.pos++;
          break;
        }
      } else if (ch === CH_BACKSLASH) {
        this.pos++;
      } else if (ch === CH_SQUOTE) {
        this.pos++;
        if (this.pos > start + 1 && src.charCodeAt(this.pos - 2) === CH_DOLLAR)
          this.skipAnsiCQuoted();
        else
          this.skipSQ();
        continue;
      } else if (ch === CH_DQUOTE) {
        this.pos++;
        this.skipDQ();
        continue;
      }
      this.pos++;
    }
    const closed = depth === 0;
    if (!closed)
      this.errors.push({ message: "unterminated parameter expansion", pos: start - 1 });
    const text = this._buildParts || this._buildValue ? src.slice(start - 1, this.pos) : "";
    this._resultText = text;
    this._resultIsRaw = true;
    this._resultHasExpansion = false;
    if (this._buildParts) {
      const inner = src.slice(start + 1, closed ? this.pos - 1 : this.pos);
      this._resultPart = this.parseParamInner(text, inner, start + 1);
    } else {
      this._resultPart = undefined;
    }
  }
  parseParamInner(text, inner, innerStart) {
    const result = {
      type: "ParameterExpansion",
      text,
      parameter: "",
      index: undefined,
      indexParts: undefined,
      indirect: undefined,
      length: undefined,
      operator: undefined,
      operand: undefined,
      slice: undefined,
      replace: undefined
    };
    const ilen = inner.length;
    if (ilen === 0)
      return result;
    const sub = (a, b) => this.parseSubFieldWord(innerStart + a, innerStart + b);
    const closeBracket = (start) => {
      const close = this.findClosingBracket(innerStart + start, innerStart + ilen);
      return close === -1 ? -1 : close - innerStart;
    };
    let i = 0;
    if (inner.charCodeAt(0) === CH_BANG) {
      result.indirect = true;
      i = 1;
    }
    if (!result.indirect && inner.charCodeAt(0) === CH_HASH) {
      if (ilen === 1) {
        result.parameter = "#";
        return result;
      }
      if (inner.charCodeAt(1) === CH_HASH) {
        result.parameter = "#";
        i = 1;
      } else {
        const tryI = this.scanParamName(inner, 1);
        if (tryI > 1) {
          let endI = tryI;
          if (endI < ilen && inner.charCodeAt(endI) === CH_LBRACKET) {
            const closeB = closeBracket(endI + 1);
            if (closeB !== -1)
              endI = closeB + 1;
          }
          if (endI >= ilen) {
            result.length = true;
            result.parameter = inner.slice(1, tryI);
            if (tryI < ilen && inner.charCodeAt(tryI) === CH_LBRACKET) {
              const closeB = closeBracket(tryI + 1);
              if (closeB !== -1) {
                result.index = inner.slice(tryI + 1, closeB);
                result.indexParts = sub(tryI + 1, closeB).parts;
              }
            }
            return result;
          }
        }
        result.parameter = "#";
        i = 1;
      }
    }
    if (!result.parameter) {
      const nameStart = i;
      i = this.scanParamName(inner, i);
      result.parameter = inner.slice(nameStart, i);
    }
    if (i < ilen && inner.charCodeAt(i) === CH_LBRACKET) {
      const closeB = closeBracket(i + 1);
      if (closeB !== -1) {
        result.index = inner.slice(i + 1, closeB);
        result.indexParts = sub(i + 1, closeB).parts;
        i = closeB + 1;
      }
    }
    if (i >= ilen)
      return result;
    const opChar = inner.charCodeAt(i);
    if (opChar === CH_COLON) {
      if (i + 1 < ilen) {
        const nc = inner.charCodeAt(i + 1);
        if (nc === CH_DASH || nc === CH_EQ || nc === CH_PLUS || nc === CH_QUESTION) {
          result.operator = inner.slice(i, i + 2);
          result.operand = sub(i + 2, ilen);
          return result;
        }
      }
      i++;
      const sliceRest = inner.slice(i);
      const sliceStart = innerStart + i;
      const sliceEnd = innerStart + ilen;
      const colonIdx = findUnnested(sliceRest, CH_COLON, true, (index, quoted) => {
        return this.findNestedShellEnd(sliceStart + index, sliceEnd, quoted) - sliceStart;
      });
      if (colonIdx === -1) {
        result.slice = { offset: sub(i, ilen), length: undefined };
      } else {
        result.slice = {
          offset: sub(i, i + colonIdx),
          length: sub(i + colonIdx + 1, ilen)
        };
      }
      return result;
    }
    if (opChar === CH_DASH || opChar === CH_EQ || opChar === CH_PLUS || opChar === CH_QUESTION) {
      result.operator = inner[i];
      result.operand = sub(i + 1, ilen);
      return result;
    }
    if (opChar === CH_HASH) {
      if (i + 1 < ilen && inner.charCodeAt(i + 1) === CH_HASH) {
        result.operator = "##";
        result.operand = sub(i + 2, ilen);
      } else {
        result.operator = "#";
        result.operand = sub(i + 1, ilen);
      }
      return result;
    }
    if (opChar === CH_PERCENT) {
      if (i + 1 < ilen && inner.charCodeAt(i + 1) === CH_PERCENT) {
        result.operator = "%%";
        result.operand = sub(i + 2, ilen);
      } else {
        result.operator = "%";
        result.operand = sub(i + 1, ilen);
      }
      return result;
    }
    if (opChar === CH_SLASH) {
      i++;
      let replOp = "/";
      if (i < ilen) {
        const nc = inner.charCodeAt(i);
        if (nc === CH_SLASH) {
          replOp = "//";
          i++;
        } else if (nc === CH_HASH) {
          replOp = "/#";
          i++;
        } else if (nc === CH_PERCENT) {
          replOp = "/%";
          i++;
        }
      }
      result.operator = replOp;
      const rest = inner.slice(i);
      const sepIdx = findUnnested(rest, CH_SLASH);
      if (sepIdx === -1) {
        result.replace = {
          pattern: sub(i, ilen),
          replacement: new WordImpl("", innerStart + ilen, innerStart + ilen)
        };
      } else {
        result.replace = {
          pattern: sub(i, i + sepIdx),
          replacement: sub(i + sepIdx + 1, ilen)
        };
      }
      return result;
    }
    if (opChar === CH_CARET) {
      if (i + 1 < ilen && inner.charCodeAt(i + 1) === CH_CARET) {
        result.operator = "^^";
        if (i + 2 < ilen)
          result.operand = sub(i + 2, ilen);
      } else {
        result.operator = "^";
        if (i + 1 < ilen)
          result.operand = sub(i + 1, ilen);
      }
      return result;
    }
    if (opChar === CH_COMMA) {
      if (i + 1 < ilen && inner.charCodeAt(i + 1) === CH_COMMA) {
        result.operator = ",,";
        if (i + 2 < ilen)
          result.operand = sub(i + 2, ilen);
      } else {
        result.operator = ",";
        if (i + 1 < ilen)
          result.operand = sub(i + 1, ilen);
      }
      return result;
    }
    if (opChar === CH_AT) {
      result.operator = "@";
      result.operand = sub(i + 1, ilen);
      return result;
    }
    result.operator = inner.slice(i);
    return result;
  }
  findNestedShellEnd(start, end, quoted) {
    const ch = this.src.charCodeAt(start);
    if (ch === CH_BACKTICK) {
      let pos = start + 1;
      while (pos < end) {
        const current = this.src.charCodeAt(pos);
        if (current === CH_BACKSLASH) {
          pos += 2;
          continue;
        }
        pos++;
        if (current === CH_BACKTICK)
          return pos;
      }
      return end;
    }
    let close;
    if (ch === CH_DOLLAR) {
      const next = start + 1;
      if (next < end && this.src.charCodeAt(next) === CH_LBRACE) {
        close = this.findClosingBrace(next + 1, end);
      } else {
        const open = skipLineContinuations(this.src, next, end);
        if (open >= end || this.src.charCodeAt(open) !== CH_LPAREN)
          return start;
        close = this.findClosingParenthesis(open + 1, end);
      }
    } else {
      const open = start + 1;
      if (quoted || ch !== CH_LT && ch !== CH_GT || open >= end || this.src.charCodeAt(open) !== CH_LPAREN) {
        return start;
      }
      close = this.findClosingParenthesis(open + 1, end);
    }
    return close === -1 ? end : close + 1;
  }
  scanParamName(s, start) {
    let i = start;
    if (i >= s.length)
      return i;
    const c = s.charCodeAt(i);
    if (c === CH_AT || c === CH_STAR || c === CH_HASH || c === CH_QUESTION || c === CH_DASH || c === CH_DOLLAR || c === CH_BANG) {
      return i + 1;
    }
    if (c >= CH_0 && c <= CH_9) {
      while (i < s.length && s.charCodeAt(i) >= CH_0 && s.charCodeAt(i) <= CH_9)
        i++;
      return i;
    }
    if (c >= CH_a && c <= CH_z || c >= CH_A && c <= CH_Z || c === CH_UNDERSCORE) {
      i++;
      while (i < s.length) {
        const ch = s.charCodeAt(i);
        if (ch >= CH_a && ch <= CH_z || ch >= CH_A && ch <= CH_Z || ch >= CH_0 && ch <= CH_9 || ch === CH_UNDERSCORE)
          i++;
        else
          break;
      }
    }
    return i;
  }
  readAnsiCQuoted() {
    const quotePos = this.pos - 1;
    const result = decodeAnsiCQuoted(this.src, this.pos, this.srcEnd);
    this.pos = result.end;
    if (!result.closed)
      this.errors.push({ message: "unterminated ANSI-C quote", pos: quotePos });
    return result.value;
  }
  extractBalanced() {
    const src = this.src;
    const len = this.srcEnd;
    const bt = this._buildParts || this._buildValue;
    let depth = 1;
    const start = this.pos;
    this._unbalanced = false;
    let wordStart = true;
    while (this.pos < len) {
      const c = src.charCodeAt(this.pos);
      if (c === CH_RPAREN) {
        const result = bt ? src.slice(start, this.pos) : "";
        this.pos++;
        return result;
      } else if (c === CH_LPAREN || c === CH_BACKSLASH || c === CH_SQUOTE || c === CH_DQUOTE || c === CH_BACKTICK) {
        break;
      } else if (c === CH_LT && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_LT) {
        break;
      } else if (c === CH_HASH && wordStart) {
        break;
      } else if (c === 99 && wordStart && this.pos + 3 < len && src.charCodeAt(this.pos + 1) === 97 && src.charCodeAt(this.pos + 2) === 115 && src.charCodeAt(this.pos + 3) === 101 && (this.pos + 4 >= len || src.charCodeAt(this.pos + 4) < 128 && charType[src.charCodeAt(this.pos + 4)] & 1)) {
        break;
      } else {
        wordStart = c < 128 && (charType[c] & 1) !== 0;
        this.pos++;
      }
    }
    let caseDepth = 0;
    let caseParens = 0;
    const wordStartAfterParen = [];
    let pendingDelims = null;
    let arithBase = -1;
    const arithExtent = start >= 2 && src.charCodeAt(start) === CH_LPAREN && src.charCodeAt(start - 1) === CH_LPAREN && src.charCodeAt(start - 2) === CH_DOLLAR;
    let substitutions = 0;
    let reported = false;
    let continuedDollarPos = -1;
    let continuedParenPos = -1;
    while (this.pos < len && depth > 0) {
      const ch = src.charCodeAt(this.pos);
      if (ch === CH_LPAREN) {
        const prev = this.pos > start ? src.charCodeAt(this.pos - 1) : 0;
        const commandDollarPos = prev === CH_DOLLAR ? this.pos - 1 : this.pos === continuedParenPos ? continuedDollarPos : -1;
        const wordParen = commandDollarPos !== -1 || prev === CH_LT || prev === CH_GT || prev === CH_EQ || prev === CH_AT || prev === CH_QUESTION || prev === CH_STAR || prev === CH_PLUS || prev === CH_BANG || prev === CH_LPAREN && wordStartAfterParen[wordStartAfterParen.length - 1] === false || arithExtent && this.pos === start;
        wordStartAfterParen.push(!wordParen);
        wordStart = true;
        if (arithBase < 0 && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_LPAREN) {
          arithBase = depth;
        }
        if (commandDollarPos !== -1 && ++substitutions + this._nestingDepth >= MAX_SYNTAX_NESTING) {
          if (!reported) {
            this.errors.push({ message: "maximum command substitution nesting depth exceeded", pos: commandDollarPos });
            reported = true;
          }
        }
        continuedDollarPos = -1;
        continuedParenPos = -1;
        depth++;
        if (caseDepth > 0)
          caseParens++;
        this.pos++;
      } else if (ch === CH_RPAREN) {
        if (caseDepth > 0 && caseParens === 0) {
          this.pos++;
          wordStart = true;
        } else {
          if (caseDepth > 0)
            caseParens--;
          depth--;
          if (depth === 0) {
            const result = bt ? src.slice(start, this.pos) : "";
            this.pos++;
            return result;
          }
          if (depth <= arithBase)
            arithBase = -1;
          wordStart = wordStartAfterParen.pop() ?? true;
          this.pos++;
        }
      } else if (ch === CH_BACKSLASH) {
        if (this.pos > start && src.charCodeAt(this.pos - 1) === CH_DOLLAR) {
          const logicalPos = skipLineContinuations(src, this.pos, len);
          if (logicalPos > this.pos && src.charCodeAt(logicalPos) === CH_LPAREN && src.charCodeAt(logicalPos + 1) !== CH_LPAREN) {
            continuedDollarPos = this.pos - 1;
            continuedParenPos = logicalPos;
          }
        }
        this.pos++;
        if (this.pos < len) {
          if (src.charCodeAt(this.pos) !== CH_NL) {
            wordStart = false;
          }
          this.pos++;
        }
      } else if (ch === CH_SQUOTE) {
        this.pos++;
        this.skipSQ();
        wordStart = false;
      } else if (ch === CH_DQUOTE) {
        this.pos++;
        this.skipDQ();
        wordStart = false;
      } else if (ch === CH_BACKTICK) {
        this.pos++;
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_BACKTICK) {
          if (src.charCodeAt(this.pos) === CH_BACKSLASH)
            this.pos++;
          if (this.pos < len)
            this.pos++;
        }
        if (this.pos < len)
          this.pos++;
        wordStart = false;
      } else if (ch === CH_LT && arithBase < 0 && this.pos + 1 < len && src.charCodeAt(this.pos + 1) === CH_LT) {
        if (this.pos + 2 < len && src.charCodeAt(this.pos + 2) === CH_LT) {
          this.pos += 3;
        } else {
          this.pos += 2;
          const strip = this.pos < len && src.charCodeAt(this.pos) === CH_DASH;
          if (strip)
            this.pos++;
          this.skipSpacesAndTabs();
          this.readHereDocDelimiter();
          if (this._hereDelim || this._hereQuoted) {
            (pendingDelims ??= []).push({ delimiter: this._hereDelim, strip, quoted: this._hereQuoted });
          }
        }
        wordStart = false;
      } else if (ch === CH_NL && pendingDelims) {
        this.pos++;
        for (const hd of pendingDelims)
          this.skipHereDocBody(hd.delimiter, hd.strip, true, hd.quoted);
        pendingDelims = null;
        wordStart = true;
      } else if (ch === CH_HASH && arithBase < 0 && !arithExtent && wordStart) {
        while (this.pos < len && src.charCodeAt(this.pos) !== CH_NL)
          this.pos++;
      } else {
        const wStart = this.pos;
        while (this.pos < len) {
          const wc = src.charCodeAt(this.pos);
          if (wc < 128 && charType[wc])
            break;
          this.pos++;
        }
        if (this.pos > wStart) {
          const wLen = this.pos - wStart;
          if (wLen === 4 && wordStart) {
            const c0 = src.charCodeAt(wStart);
            if (c0 === 99 && src.charCodeAt(wStart + 1) === 97 && src.charCodeAt(wStart + 2) === 115 && src.charCodeAt(wStart + 3) === 101) {
              caseDepth++;
            } else if (c0 === 101 && src.charCodeAt(wStart + 1) === 115 && src.charCodeAt(wStart + 2) === 97 && src.charCodeAt(wStart + 3) === 99 && caseDepth > 0) {
              caseDepth--;
              if (caseDepth === 0)
                caseParens = 0;
            }
          }
          wordStart = false;
        } else {
          const wc = src.charCodeAt(this.pos);
          wordStart = wc < 128 && (charType[wc] & 1) !== 0;
          this.pos++;
        }
      }
    }
    this._unbalanced = true;
    return bt ? src.slice(start, this.pos) : "";
  }
}

// node_modules/.bun/unbash@4.0.11/node_modules/unbash/dist/parts.js
function computeWordParts(source, word, depth = 0) {
  const lexer = new Lexer(source, word.pos, word.end);
  lexer._nestingDepth = depth;
  const parts = lexer.buildWordParts(word.pos);
  if (!parts)
    return;
  resolveCollected(lexer);
  return parts;
}
function computeEmbeddedWordParts(source, word, depth = 0) {
  if (!hasEmbeddedWordStructure(source, word.pos, word.end))
    return;
  const lexer = new Lexer(source, word.pos, word.end);
  lexer._nestingDepth = depth;
  const parts = lexer.buildEmbeddedWordParts(word.pos);
  if (!parts)
    return;
  resolveCollected(lexer);
  return parts;
}
function computeHereDocBodyParts(source, word, depth = 0) {
  const lexer = new Lexer(source, word.pos, word.end);
  lexer._nestingDepth = depth;
  const parts = lexer.buildHereDocParts(word.pos, word.end);
  if (!parts)
    return;
  resolveCollected(lexer);
  return parts;
}
function resolveCollected(lexer) {
  const source = lexer.getSource();
  for (const [e, innerDepth] of lexer.getCollectedExpansions()) {
    if (e.inner !== undefined) {
      const depth = innerDepth + 1;
      if (depth > MAX_SYNTAX_NESTING + 1) {} else if (e.innerStart !== undefined) {
        e.script = parseRegion(source, e.innerStart, e.innerStart + e.inner.length, depth);
      } else {
        e.script = parse4(e.inner);
        Object.defineProperty(e.script, "source", { value: e.inner, enumerable: false });
      }
      e.inner = undefined;
      e.innerStart = undefined;
    }
  }
}

// node_modules/.bun/unbash@4.0.11/node_modules/unbash/dist/parser.js
WordImpl._resolveWord = computeWordParts;
WordImpl._resolveHeredocBody = computeHereDocBodyParts;

class ArithmeticCommandImpl {
  type = "ArithmeticCommand";
  pos;
  end;
  body;
  #source;
  #depth;
  #expression = null;
  constructor(pos, end, body, source, depth) {
    this.pos = pos;
    this.end = end;
    this.body = body;
    this.#source = source;
    this.#depth = depth;
  }
  get expression() {
    if (this.#expression === null) {
      this.#expression = parseArithmeticWithParts(this.body, this.pos + 2, this.#source, this.#depth);
    }
    return this.#expression;
  }
  set expression(v) {
    this.#expression = v ?? undefined;
  }
  toJSON() {
    return {
      type: this.type,
      pos: this.pos,
      end: this.end,
      expression: this.expression,
      body: this.body
    };
  }
}

class ArithmeticForImpl {
  type = "ArithmeticFor";
  pos;
  end;
  body;
  #initStr;
  #testStr;
  #updateStr;
  #initPos;
  #testPos;
  #updatePos;
  #source;
  #depth;
  #initialize = null;
  #test = null;
  #update = null;
  constructor(pos, end, body, initStr, testStr, updateStr, initPos, testPos, updatePos, source, depth) {
    this.pos = pos;
    this.end = end;
    this.body = body;
    this.#initStr = initStr;
    this.#testStr = testStr;
    this.#updateStr = updateStr;
    this.#initPos = initPos;
    this.#testPos = testPos;
    this.#updatePos = updatePos;
    this.#source = source;
    this.#depth = depth;
  }
  get initialize() {
    if (this.#initialize === null) {
      if (this.#initStr) {
        this.#initialize = parseArithmeticWithParts(this.#initStr, this.#initPos, this.#source, this.#depth);
      } else {
        this.#initialize = undefined;
      }
    }
    return this.#initialize;
  }
  set initialize(v) {
    this.#initialize = v ?? undefined;
  }
  get test() {
    if (this.#test === null) {
      if (this.#testStr) {
        this.#test = parseArithmeticWithParts(this.#testStr, this.#testPos, this.#source, this.#depth);
      } else {
        this.#test = undefined;
      }
    }
    return this.#test;
  }
  set test(v) {
    this.#test = v ?? undefined;
  }
  get update() {
    if (this.#update === null) {
      if (this.#updateStr) {
        this.#update = parseArithmeticWithParts(this.#updateStr, this.#updatePos, this.#source, this.#depth);
      } else {
        this.#update = undefined;
      }
    }
    return this.#update;
  }
  set update(v) {
    this.#update = v ?? undefined;
  }
  toJSON() {
    return {
      type: this.type,
      pos: this.pos,
      end: this.end,
      initialize: this.initialize,
      test: this.test,
      update: this.update,
      body: this.body
    };
  }
}
var CASE_TERMINATORS = {
  [Token.DoubleSemi]: ";;",
  [Token.SemiAmp]: ";&",
  [Token.DoubleSemiAmp]: ";;&"
};
var REDIRECT_OPS = {
  ">": ">",
  ">>": ">>",
  "<": "<",
  "<<": "<<",
  "<<-": "<<-",
  "<<<": "<<<",
  "<>": "<>",
  "<&": "<&",
  ">&": ">&",
  ">|": ">|",
  "&>": "&>",
  "&>>": "&>>"
};
function parseArithmeticWithParts(body, offset, source, depth = 0) {
  if (!hasEmbeddedWordStructure(source, offset, offset + body.length)) {
    return parseArithmeticExpression(body, offset) ?? undefined;
  }
  const commandExpansions = [];
  const embeddedWords = [];
  const lexer = new Lexer(source);
  const expression = parseArithmeticExpression(body, offset, {
    commandExpansions,
    embeddedWords,
    findClosingBracket: (start, end) => lexer.findClosingBracket(start, end),
    findClosingBrace: (start, end) => lexer.findClosingBrace(start, end),
    findClosingParenthesis: (start, end) => lexer.findClosingParenthesis(start, end),
    findArithmeticExpansionEnd: (start, end) => lexer.findArithmeticExpansionEnd(start, end),
    findArithmeticWordEnd: (start, end) => lexer.findArithmeticWordEnd(start, end)
  }) ?? undefined;
  for (const node of commandExpansions) {
    if (node.inner !== undefined) {
      if (depth <= MAX_SYNTAX_NESTING) {
        const innerStart = node.pos + 2;
        node.script = parseRegion(source, innerStart, innerStart + node.inner.length, depth + 1);
      }
      node.inner = undefined;
    }
  }
  for (const node of embeddedWords)
    node.parts = computeEmbeddedWordParts(source, node, depth);
  return expression;
}
var listTerminators = new Uint8Array(37);
listTerminators[Token.EOF] = 1;
listTerminators[Token.RParen] = 1;
listTerminators[Token.RBrace] = 1;
listTerminators[Token.Then] = 1;
listTerminators[Token.Else] = 1;
listTerminators[Token.Elif] = 1;
listTerminators[Token.Fi] = 1;
listTerminators[Token.Do] = 1;
listTerminators[Token.Done] = 1;
listTerminators[Token.Esac] = 1;
listTerminators[Token.DoubleSemi] = 1;
listTerminators[Token.SemiAmp] = 1;
listTerminators[Token.DoubleSemiAmp] = 1;
var compoundClosers = new Uint8Array(37);
compoundClosers[Token.RParen] = 1;
compoundClosers[Token.RBrace] = 1;
compoundClosers[Token.DblRBracket] = 1;
compoundClosers[Token.Fi] = 1;
compoundClosers[Token.Done] = 1;
compoundClosers[Token.Esac] = 1;
compoundClosers[Token.ArithCmd] = 1;
function isTestNegation(t) {
  return t.token === Token.Word && t.keywordEligible && t.value === "!";
}
var commandStarts = new Uint8Array(37);
commandStarts[Token.Word] = 1;
commandStarts[Token.Assignment] = 1;
commandStarts[Token.Bang] = 1;
commandStarts[Token.LParen] = 1;
commandStarts[Token.LBrace] = 1;
commandStarts[Token.DblLBracket] = 1;
commandStarts[Token.If] = 1;
commandStarts[Token.For] = 1;
commandStarts[Token.While] = 1;
commandStarts[Token.Until] = 1;
commandStarts[Token.Case] = 1;
commandStarts[Token.Function] = 1;
commandStarts[Token.Select] = 1;
commandStarts[Token.ArithCmd] = 1;
commandStarts[Token.Coproc] = 1;
commandStarts[Token.Redirect] = 1;
var UNARY_TEST_OPS = {
  "-a": 1,
  "-b": 1,
  "-c": 1,
  "-d": 1,
  "-e": 1,
  "-f": 1,
  "-g": 1,
  "-h": 1,
  "-k": 1,
  "-p": 1,
  "-r": 1,
  "-s": 1,
  "-t": 1,
  "-u": 1,
  "-v": 1,
  "-w": 1,
  "-x": 1,
  "-z": 1,
  "-n": 1,
  "-o": 1,
  "-N": 1,
  "-S": 1,
  "-L": 1,
  "-G": 1,
  "-O": 1,
  "-R": 1
};
var BINARY_TEST_OPS = {
  "==": 1,
  "!=": 1,
  "=~": 1,
  "=": 1,
  "-eq": 1,
  "-ne": 1,
  "-lt": 1,
  "-le": 1,
  "-gt": 1,
  "-ge": 1,
  "-nt": 1,
  "-ot": 1,
  "-ef": 1,
  "<": 1,
  ">": 1
};
function heredocDelimiterParts(value) {
  return (source, word) => {
    const raw = source.slice(word.pos, word.end);
    return raw === value ? undefined : [{ type: "Literal", value, text: raw }];
  };
}
var EMPTY_REDIRECTS = [];
function ownEmpty(values) {
  return values.length === 0 ? [] : values;
}
function parse4(source) {
  return new Parser(source, 0, source.length).run();
}
function parseRegion(source, start, end, depth = 0) {
  return new Parser(source, start, end, depth).run();
}

class Parser {
  tok;
  source;
  start;
  end;
  depth;
  errors = null;
  _redirects = EMPTY_REDIRECTS;
  syntaxDepth = 0;
  constructor(source, start, end, depth = 0) {
    this.tok = new Lexer(source, start, end);
    this.tok._nestingDepth = depth;
    this.source = source;
    this.start = start;
    this.end = end;
    this.depth = depth;
  }
  run() {
    const start = this.start;
    if (this.depth > MAX_SYNTAX_NESTING)
      this.error("maximum substitution nesting depth exceeded", start);
    let shebang;
    if (start === 0 && this.source.charCodeAt(0) === 35 && this.source.charCodeAt(1) === 33) {
      const nl = this.source.indexOf(`
`);
      shebang = nl === -1 ? this.source : this.source.slice(0, nl);
    }
    const commands = this.list();
    for (;; ) {
      const unexpected = this.tok.peek(LexContext.CommandStart);
      if (unexpected.token === Token.EOF)
        break;
      this.error(`unexpected token '${unexpected.value}'`, unexpected.pos);
      if (!listTerminators[unexpected.token] && unexpected.token !== Token.In)
        break;
      this.tok.next(LexContext.CommandStart);
      let separator = this.tok.peek(LexContext.CommandStart).token;
      if (separator !== Token.Semi && separator !== Token.Newline && separator !== Token.Amp)
        break;
      while (separator === Token.Semi || separator === Token.Newline || separator === Token.Amp) {
        this.tok.next(LexContext.CommandStart);
        separator = this.tok.peek(LexContext.CommandStart).token;
      }
      const recovered = this.list();
      for (let i = 0;i < recovered.length; i++)
        commands.push(recovered[i]);
    }
    const lexerErrors = this.tok._errors;
    if (lexerErrors !== null && lexerErrors.length > 0) {
      const errors = this.errors ??= [];
      for (let i = 0;i < lexerErrors.length; i++)
        errors.push(lexerErrors[i]);
    }
    if (this.errors !== null && this.errors.length > 1)
      this.errors.sort((a, b) => a.pos - b.pos);
    const result = {
      type: "Script",
      pos: start,
      end: this.end,
      shebang,
      commands,
      errors: this.errors ?? undefined
    };
    return result;
  }
  error(message, pos) {
    (this.errors ??= []).push({ message, pos });
  }
  skipSemi() {
    if (this.tok.peek(LexContext.Normal).token === Token.Semi)
      this.tok.next(LexContext.Normal);
  }
  accept(token, ctx = LexContext.Normal) {
    if (this.tok.peek(ctx).token === token)
      return this.tok.next(ctx);
    return null;
  }
  acceptEnd(token, ctx = LexContext.Normal) {
    if (this.tok.peek(ctx).token === token)
      return this.tok.next(ctx).end;
    return -1;
  }
  skipNewlines(ctx = LexContext.Normal) {
    while (this.tok.peek(ctx).token === Token.Newline)
      this.tok.next(ctx);
  }
  makeStatement(command, redirects) {
    const end = redirects.length > 0 ? redirects[redirects.length - 1].end : command.end;
    return {
      type: "Statement",
      pos: command.pos,
      end,
      command,
      background: undefined,
      redirects: ownEmpty(redirects)
    };
  }
  list() {
    const commands = [];
    this.skipNewlines(LexContext.CommandStart);
    let t = this.tok.peek(LexContext.CommandStart).token;
    if (listTerminators[t] || !commandStarts[t])
      return commands;
    const first = this.andOr();
    if (first) {
      const redirects = this._redirects;
      this._redirects = EMPTY_REDIRECTS;
      commands.push(this.makeStatement(first, redirects));
    }
    for (;; ) {
      t = this.tok.peekFollow(compoundClosers).token;
      if (t !== Token.Semi && t !== Token.Newline && t !== Token.Amp)
        break;
      const isBackground = t === Token.Amp;
      const sepEnd = this.tok.next(LexContext.Normal).end;
      if (isBackground) {
        const stmt = commands[commands.length - 1];
        stmt.background = true;
        stmt.end = sepEnd;
      }
      this.skipNewlines(LexContext.CommandStart);
      t = this.tok.peek(LexContext.CommandStart).token;
      if (listTerminators[t] || !commandStarts[t])
        break;
      const node = this.andOr();
      if (node) {
        const redirects = this._redirects;
        this._redirects = EMPTY_REDIRECTS;
        commands.push(this.makeStatement(node, redirects));
      }
    }
    return commands;
  }
  andOr() {
    const first = this.pipeline();
    if (!first)
      return null;
    let t = this.tok.peek(LexContext.Normal).token;
    if (t !== Token.And && t !== Token.Or)
      return first;
    let wrappedFirst = first;
    if (this._redirects.length > 0) {
      wrappedFirst = this.makeStatement(first, this._redirects);
      this._redirects = EMPTY_REDIRECTS;
    }
    const commands = [wrappedFirst];
    const operators = [];
    do {
      const operatorToken = this.tok.next(LexContext.Normal);
      const operator = operatorToken.token === Token.And ? "&&" : "||";
      this.skipNewlines(LexContext.CommandStart);
      const next = this.pipeline();
      if (!next) {
        this.error(`expected command after '${operator}'`, operatorToken.end);
        break;
      }
      operators.push(operator);
      commands.push(next);
      t = this.tok.peek(LexContext.Normal).token;
    } while (t === Token.And || t === Token.Or);
    return {
      type: "AndOr",
      pos: first.pos,
      end: commands[commands.length - 1].end,
      commands,
      operators
    };
  }
  wrapCompoundRedirects(node) {
    const redirects = this._redirects;
    this._redirects = EMPTY_REDIRECTS;
    if (redirects.length === 0)
      return node;
    return this.makeStatement(node, redirects);
  }
  pipeline() {
    let time = false;
    let pipelinePos = 0;
    let prefixEnd = 0;
    const firstToken = this.tok.peek(LexContext.CommandStart);
    if (firstToken.token === Token.Word && firstToken.keywordEligible && firstToken.value === "time") {
      time = true;
      const timeToken = this.tok.next(LexContext.CommandStart);
      pipelinePos = timeToken.pos;
      prefixEnd = timeToken.end;
      const flag = this.tok.peek(LexContext.CommandStart);
      if (flag.token === Token.Word && flag.keywordEligible && flag.value === "-p")
        prefixEnd = this.tok.next(LexContext.CommandStart).end;
    }
    let negated = false;
    const bang = this.tok.peek(LexContext.CommandStart);
    if (bang.token === Token.Bang) {
      if (!time)
        pipelinePos = bang.pos;
      prefixEnd = this.tok.next(LexContext.CommandStart).end;
      negated = true;
      const repeated = this.tok.peek(LexContext.CommandStart);
      if (repeated.token === Token.Bang) {
        this.error("unexpected token '!'", repeated.pos);
        do {
          prefixEnd = this.tok.next(LexContext.CommandStart).end;
        } while (this.tok.peek(LexContext.CommandStart).token === Token.Bang);
      }
    }
    const first = this.command();
    if (!first) {
      if (time || negated) {
        const pipeline = {
          type: "Pipeline",
          pos: pipelinePos,
          end: prefixEnd,
          commands: [],
          negated: negated ? true : undefined,
          operators: [],
          time: time ? true : undefined
        };
        return pipeline;
      }
      return null;
    }
    if (!time && !negated)
      pipelinePos = first.pos;
    const commands = [first];
    const operators = [];
    let firstRedirects = this._redirects;
    this._redirects = EMPTY_REDIRECTS;
    while (this.tok.peek(LexContext.Normal).token === Token.Pipe) {
      if (commands.length === 1 && firstRedirects.length > 0) {
        commands[0] = this.makeStatement(first, firstRedirects);
        firstRedirects = [];
      }
      const pipeToken = this.tok.next(LexContext.Normal);
      const operator = pipeToken.value === "|&" ? "|&" : "|";
      this.skipNewlines(LexContext.CommandStart);
      const cmd = this.command();
      if (!cmd) {
        this.error(`expected command after '${operator}'`, pipeToken.end);
        break;
      }
      operators.push(operator);
      commands.push(this.wrapCompoundRedirects(cmd));
    }
    if (commands.length === 1 && !negated && !time) {
      this._redirects = firstRedirects;
      return commands[0];
    }
    if (firstRedirects.length > 0) {
      commands[0] = this.makeStatement(first, firstRedirects);
    }
    const pipeline = {
      type: "Pipeline",
      pos: pipelinePos,
      end: commands[commands.length - 1].end,
      commands,
      negated: negated ? true : undefined,
      operators,
      time: time ? true : undefined
    };
    return pipeline;
  }
  command() {
    switch (this.tok.peek(LexContext.CommandStart).token) {
      case Token.LParen:
        return this.subshell();
      case Token.LBrace:
        return this.braceGroup();
      case Token.If:
        return this.ifClause();
      case Token.For:
        return this.forClause();
      case Token.While:
        return this.whileClause();
      case Token.Until:
        return this.untilClause();
      case Token.Case:
        return this.caseClause();
      case Token.Function:
        return this.functionDef();
      case Token.Select:
        return this.selectClause();
      case Token.DblLBracket:
        return this.testCommand();
      case Token.ArithCmd:
        return this.arithCommand();
      case Token.Coproc:
        return this.coprocCommand();
      case Token.Word:
      case Token.Assignment:
      case Token.Redirect:
        return this.simpleCommandOrFunction();
      default:
        return null;
    }
  }
  collectTrailingRedirects() {
    let redirects = EMPTY_REDIRECTS;
    while (this.tok.peekFollow(compoundClosers).token === Token.Redirect) {
      redirects = this.collectRedirect(redirects, LexContext.Normal);
    }
    return redirects;
  }
  arithCommand() {
    const tok = this.tok.next(LexContext.CommandStart);
    this._redirects = this.collectTrailingRedirects();
    return new ArithmeticCommandImpl(tok.pos, tok.end, tok.value, this.source, this.depth);
  }
  coprocCommand() {
    const startTok = this.tok.next(LexContext.CommandStart);
    const pos = startTok.pos;
    const startEnd = startTok.end;
    const t = this.tok.peek(LexContext.CommandStart);
    if (t.token !== Token.Word && t.token !== Token.Assignment && t.token !== Token.Redirect) {
      const body = this.pipeline() ?? {
        type: "Command",
        pos,
        end: startEnd,
        name: undefined,
        prefix: [],
        suffix: [],
        redirects: []
      };
      const bodyRedirects = this._redirects;
      this._redirects = EMPTY_REDIRECTS;
      const redirects = this.collectTrailingRedirects();
      const allRedirects = [...bodyRedirects, ...redirects];
      const end = allRedirects.length > 0 ? allRedirects[allRedirects.length - 1].end : body.end;
      return { type: "Coproc", pos, end, name: undefined, body, redirects: allRedirects };
    }
    const tentativeWord = this.toWord(this.tok.next(LexContext.CommandStart));
    const body = this.pipeline();
    if (body === null) {
      const cmd = {
        type: "Command",
        pos: tentativeWord.pos,
        end: tentativeWord.end,
        name: tentativeWord,
        prefix: [],
        suffix: [],
        redirects: []
      };
      const redirects = this.collectTrailingRedirects();
      const end = redirects.length > 0 ? redirects[redirects.length - 1].end : cmd.end;
      return { type: "Coproc", pos, end, name: undefined, body: cmd, redirects: ownEmpty(redirects) };
    }
    if (body.type === "Command") {
      const cmd = body;
      if (cmd.name) {
        cmd.suffix = [cmd.name, ...cmd.suffix];
      }
      cmd.name = tentativeWord;
      cmd.pos = tentativeWord.pos;
      const redirects = this.collectTrailingRedirects();
      const end = redirects.length > 0 ? redirects[redirects.length - 1].end : cmd.end;
      return { type: "Coproc", pos, end, name: undefined, body: cmd, redirects: ownEmpty(redirects) };
    }
    const bodyRedirects = this._redirects;
    this._redirects = EMPTY_REDIRECTS;
    const redirects = this.collectTrailingRedirects();
    const allRedirects = [...bodyRedirects, ...redirects];
    const end = allRedirects.length > 0 ? allRedirects[allRedirects.length - 1].end : body.end;
    return { type: "Coproc", pos, end, name: tentativeWord, body, redirects: allRedirects };
  }
  subshell() {
    return this.subshellBody(this.tok.next(LexContext.CommandStart).pos);
  }
  subshellBody(pos) {
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error("maximum subshell nesting depth exceeded", pos);
      const closeEnd = this.tok.skipSubshellBody();
      if (closeEnd < 0)
        this.error("expected ')' to close subshell", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return { type: "Subshell", pos, end, body: this.makeCompoundList([]) };
    }
    this.syntaxDepth++;
    const commands = this.list();
    this.syntaxDepth--;
    const closeEnd = this.acceptEnd(Token.RParen, LexContext.Normal);
    if (closeEnd < 0)
      this.error("expected ')' to close subshell", this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this._redirects = this.collectTrailingRedirects();
    return { type: "Subshell", pos, end, body: this.makeCompoundList(commands) };
  }
  braceGroup() {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error("maximum brace group nesting depth exceeded", pos);
      const closeEnd = this.tok.skipCompoundBody(Token.RBrace);
      if (closeEnd < 0)
        this.error("expected '}' to close brace group", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return { type: "BraceGroup", pos, end, body: this.makeCompoundList([]) };
    }
    this.syntaxDepth++;
    const commands = this.list();
    this.syntaxDepth--;
    const closeEnd = this.acceptEnd(Token.RBrace, LexContext.Normal);
    if (closeEnd < 0)
      this.error("expected '}' to close brace group", this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this._redirects = this.collectTrailingRedirects();
    return { type: "BraceGroup", pos, end, body: this.makeCompoundList(commands) };
  }
  ifClause() {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error("maximum if nesting depth exceeded", pos);
      const closeEnd = this.tok.skipCompoundBody(Token.Fi);
      if (closeEnd < 0)
        this.error("expected 'fi' to close 'if'", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return {
        type: "If",
        pos,
        end,
        clause: this.makeCompoundList([]),
        then: this.makeCompoundList([]),
        else: undefined
      };
    }
    this.syntaxDepth++;
    let firstBranch;
    let lastBranch;
    let branchPos = pos;
    let clause;
    let then_;
    for (;; ) {
      clause = this.makeCompoundList(this.list());
      this.skipSemi();
      const thenToken = this.accept(Token.Then, LexContext.CommandStart);
      if (!thenToken)
        this.error("expected 'then'", this.tok.getPos());
      const thenCommands = this.list();
      if (thenToken && thenCommands.length === 0)
        this.error("expected command after 'then'", this.tok.peek(LexContext.CommandStart).pos);
      then_ = this.makeCompoundList(thenCommands);
      this.skipSemi();
      const elif = this.accept(Token.Elif, LexContext.CommandStart);
      if (!elif)
        break;
      const branch = {
        type: "If",
        pos: branchPos,
        end: branchPos,
        clause,
        then: then_,
        else: undefined
      };
      if (lastBranch)
        lastBranch.else = branch;
      else
        firstBranch = branch;
      lastBranch = branch;
      branchPos = elif.pos;
    }
    let else_;
    let end;
    if (this.accept(Token.Else, LexContext.CommandStart)) {
      else_ = this.makeCompoundList(this.list());
      this.skipSemi();
      const closeEnd = this.acceptEnd(Token.Fi, LexContext.CommandStart);
      if (closeEnd < 0)
        this.error("expected 'fi' to close 'if'", this.tok.getPos());
      end = closeEnd >= 0 ? closeEnd : branchPos;
    } else {
      const closeEnd = this.acceptEnd(Token.Fi, LexContext.CommandStart);
      if (closeEnd < 0)
        this.error("expected 'fi' to close 'if'", this.tok.getPos());
      end = closeEnd >= 0 ? closeEnd : branchPos;
    }
    this.syntaxDepth--;
    this._redirects = this.collectTrailingRedirects();
    const finalBranch = { type: "If", pos: branchPos, end, clause, then: then_, else: else_ };
    if (!firstBranch)
      return finalBranch;
    lastBranch.else = finalBranch;
    let branch = firstBranch;
    while (branch !== finalBranch) {
      branch.end = end;
      branch = branch.else;
    }
    return firstBranch;
  }
  forClause() {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    if (this.tok.peek(LexContext.Normal).token === Token.LParen) {
      return this.cStyleFor(pos);
    }
    const name = this.readWord(LexContext.Normal);
    const wordlist = [];
    this.skipNewlines(LexContext.CommandStart);
    if (this.tok.peek(LexContext.CommandStart).token === Token.In) {
      this.tok.next(LexContext.CommandStart);
      while (this.tok.peek(LexContext.Normal).token === Token.Word) {
        wordlist.push(this.readWord(LexContext.Normal));
      }
    }
    this.skipSemi();
    this.skipNewlines(LexContext.CommandStart);
    if (this.tok.peek(LexContext.CommandStart).token === Token.LBrace) {
      const bg = this.braceGroup();
      return { type: "For", pos, end: bg.end, name, wordlist, body: bg.body };
    }
    if (!this.accept(Token.Do, LexContext.CommandStart))
      this.error("expected 'do'", this.tok.getPos());
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error("maximum for nesting depth exceeded", pos);
      const closeEnd = this.tok.skipCompoundBody(Token.Done);
      if (closeEnd < 0)
        this.error("expected 'done' to close 'for'", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return { type: "For", pos, end, name, wordlist, body: this.makeCompoundList([]) };
    }
    this.syntaxDepth++;
    const body = this.list();
    this.syntaxDepth--;
    this.skipSemi();
    const closeEnd = this.acceptEnd(Token.Done, LexContext.CommandStart);
    if (closeEnd < 0)
      this.error("expected 'done' to close 'for'", this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this._redirects = this.collectTrailingRedirects();
    return { type: "For", pos, end, name, wordlist, body: this.makeCompoundList(body) };
  }
  cStyleFor(pos) {
    const [initStr, testStr, updateStr, initPos, testPos, updatePos] = this.tok.readCStyleForExprs();
    if (this.tok.peek(LexContext.CommandStart).token === Token.Semi)
      this.tok.next(LexContext.CommandStart);
    this.skipNewlines(LexContext.CommandStart);
    if (this.tok.peek(LexContext.CommandStart).token === Token.LBrace) {
      const bg = this.braceGroup();
      return new ArithmeticForImpl(pos, bg.end, bg.body, initStr, testStr, updateStr, initPos, testPos, updatePos, this.source, this.depth);
    }
    if (!this.accept(Token.Do, LexContext.CommandStart))
      this.error("expected 'do'", this.tok.getPos());
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error("maximum for nesting depth exceeded", pos);
      const closeEnd = this.tok.skipCompoundBody(Token.Done);
      if (closeEnd < 0)
        this.error("expected 'done' to close 'for'", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return new ArithmeticForImpl(pos, end, this.makeCompoundList([]), initStr, testStr, updateStr, initPos, testPos, updatePos, this.source, this.depth);
    }
    this.syntaxDepth++;
    const body = this.list();
    this.syntaxDepth--;
    const closeEnd = this.acceptEnd(Token.Done, LexContext.CommandStart);
    if (closeEnd < 0)
      this.error("expected 'done' to close 'for'", this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this._redirects = this.collectTrailingRedirects();
    return new ArithmeticForImpl(pos, end, this.makeCompoundList(body), initStr, testStr, updateStr, initPos, testPos, updatePos, this.source, this.depth);
  }
  whileClause() {
    return this.whileOrUntil("while");
  }
  untilClause() {
    return this.whileOrUntil("until");
  }
  whileOrUntil(kind) {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error(`maximum ${kind} nesting depth exceeded`, pos);
      const closeEnd = this.tok.skipCompoundBody(Token.Done);
      if (closeEnd < 0)
        this.error(`expected 'done' to close '${kind}'`, this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return {
        type: "While",
        pos,
        end,
        kind,
        clause: this.makeCompoundList([]),
        body: this.makeCompoundList([])
      };
    }
    this.syntaxDepth++;
    const clause = this.makeCompoundList(this.list());
    this.skipSemi();
    if (!this.accept(Token.Do, LexContext.CommandStart))
      this.error("expected 'do'", this.tok.getPos());
    const body = this.list();
    this.skipSemi();
    const closeEnd = this.acceptEnd(Token.Done, LexContext.CommandStart);
    if (closeEnd < 0)
      this.error(`expected 'done' to close '${kind}'`, this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this.syntaxDepth--;
    this._redirects = this.collectTrailingRedirects();
    return { type: "While", pos, end, kind, clause, body: this.makeCompoundList(body) };
  }
  caseClause() {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    const word = this.readWord(LexContext.Normal);
    this.skipNewlines(LexContext.CommandStart);
    if (!this.accept(Token.In, LexContext.CommandStart))
      this.error("expected 'in' after 'case' word", this.tok.getPos());
    this.skipNewlines(LexContext.CommandStart);
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error("maximum case nesting depth exceeded", pos);
      const closeEnd = this.tok.skipCompoundBody(Token.Esac);
      if (closeEnd < 0)
        this.error("expected 'esac' to close 'case'", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return { type: "Case", pos, end, word, items: [] };
    }
    this.syntaxDepth++;
    const items = [];
    let t = this.tok.peek(LexContext.CommandStart).token;
    while (t !== Token.Esac && t !== Token.EOF) {
      const itemPos = this.tok.peek(LexContext.Normal).pos;
      this.accept(Token.LParen, LexContext.Normal);
      const pattern = [];
      t = this.tok.peek(LexContext.Normal).token;
      while (t !== Token.RParen && t !== Token.EOF) {
        if (t !== Token.Pipe)
          pattern.push(this.toWord(this.tok.next(LexContext.Normal)));
        else
          this.tok.next(LexContext.Normal);
        t = this.tok.peek(LexContext.Normal).token;
      }
      const rparenEnd = this.acceptEnd(Token.RParen, LexContext.Normal);
      const cmds = this.list();
      let itemEnd = rparenEnd >= 0 ? rparenEnd : itemPos;
      if (cmds.length > 0)
        itemEnd = cmds[cmds.length - 1].end;
      const item = {
        type: "CaseItem",
        pos: itemPos,
        end: itemEnd,
        pattern,
        body: this.makeCompoundList(cmds),
        terminator: undefined
      };
      t = this.tok.peek(LexContext.CommandStart).token;
      if (t === Token.DoubleSemi || t === Token.SemiAmp || t === Token.DoubleSemiAmp) {
        const termTok = this.tok.next(LexContext.CommandStart);
        item.terminator = CASE_TERMINATORS[termTok.token];
        item.end = termTok.end;
      }
      items.push(item);
      this.skipNewlines(LexContext.CommandStart);
      t = this.tok.peek(LexContext.CommandStart).token;
    }
    const closeEnd = this.acceptEnd(Token.Esac, LexContext.CommandStart);
    if (closeEnd < 0)
      this.error("expected 'esac' to close 'case'", this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this.syntaxDepth--;
    this._redirects = this.collectTrailingRedirects();
    return { type: "Case", pos, end, word, items };
  }
  selectClause() {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    const name = this.readWord(LexContext.Normal);
    const wordlist = [];
    this.skipNewlines(LexContext.CommandStart);
    if (this.tok.peek(LexContext.CommandStart).token === Token.In) {
      this.tok.next(LexContext.CommandStart);
      while (this.tok.peek(LexContext.Normal).token === Token.Word) {
        wordlist.push(this.readWord(LexContext.Normal));
      }
    }
    this.skipSemi();
    this.skipNewlines(LexContext.CommandStart);
    if (this.tok.peek(LexContext.CommandStart).token === Token.LBrace) {
      const bg = this.braceGroup();
      return { type: "Select", pos, end: bg.end, name, wordlist, body: bg.body };
    }
    if (!this.accept(Token.Do, LexContext.CommandStart))
      this.error("expected 'do'", this.tok.getPos());
    if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
      this.error("maximum select nesting depth exceeded", pos);
      const closeEnd = this.tok.skipCompoundBody(Token.Done);
      if (closeEnd < 0)
        this.error("expected 'done' to close 'select'", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : pos;
      this._redirects = this.collectTrailingRedirects();
      return { type: "Select", pos, end, name, wordlist, body: this.makeCompoundList([]) };
    }
    this.syntaxDepth++;
    const body = this.list();
    this.syntaxDepth--;
    this.skipSemi();
    const closeEnd = this.acceptEnd(Token.Done, LexContext.CommandStart);
    if (closeEnd < 0)
      this.error("expected 'done' to close 'select'", this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this._redirects = this.collectTrailingRedirects();
    return { type: "Select", pos, end, name, wordlist, body: this.makeCompoundList(body) };
  }
  testCommand() {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    const expr = this.parseTestOr();
    const closeEnd = this.acceptEnd(Token.DblRBracket, LexContext.TestMode);
    if (closeEnd < 0)
      this.error("expected ']]' to close '[['", this.tok.getPos());
    const end = closeEnd >= 0 ? closeEnd : pos;
    this._redirects = this.collectTrailingRedirects();
    return { type: "TestCommand", pos, end, expression: expr };
  }
  parseTestOr() {
    let left = this.parseTestAnd();
    while (this.tok.peek(LexContext.TestMode).token === Token.Or) {
      this.tok.next(LexContext.TestMode);
      const right = this.parseTestAnd();
      left = {
        type: "TestLogical",
        pos: left.pos,
        end: right.end,
        operator: "||",
        left,
        right
      };
    }
    return left;
  }
  parseTestAnd() {
    let left = this.parseTestNot();
    while (this.tok.peek(LexContext.TestMode).token === Token.And) {
      this.tok.next(LexContext.TestMode);
      const right = this.parseTestNot();
      left = {
        type: "TestLogical",
        pos: left.pos,
        end: right.end,
        operator: "&&",
        left,
        right
      };
    }
    return left;
  }
  parseTestNot() {
    let t = this.tok.peek(LexContext.TestMode);
    if (!isTestNegation(t))
      return this.parseTestPrimary();
    const firstPos = this.tok.next(LexContext.TestMode).pos;
    t = this.tok.peek(LexContext.TestMode);
    if (!isTestNegation(t)) {
      const operand = this.parseTestPrimary();
      return { type: "TestNot", pos: firstPos, end: operand.end, operand };
    }
    const positions = [firstPos];
    while (isTestNegation(t)) {
      positions.push(this.tok.next(LexContext.TestMode).pos);
      t = this.tok.peek(LexContext.TestMode);
    }
    let expression = this.parseTestPrimary();
    for (let i = positions.length - 1;i >= 0; i--) {
      expression = {
        type: "TestNot",
        pos: positions[i],
        end: expression.end,
        operand: expression
      };
    }
    return expression;
  }
  parseTestPrimary() {
    if (this.tok.peek(LexContext.TestMode).token === Token.LParen) {
      const openPos = this.tok.next(LexContext.TestMode).pos;
      if (this.syntaxDepth === MAX_SYNTAX_NESTING) {
        this.error("maximum test group nesting depth exceeded", openPos);
        const closeEnd = this.tok.skipTestGroup();
        if (closeEnd < 0)
          this.error("expected ')' to close test group", this.tok.getPos());
        const end = closeEnd >= 0 ? closeEnd : openPos;
        const operand = new WordImpl("", openPos, openPos, this.source, undefined, this.depth);
        const expression = {
          type: "TestUnary",
          pos: openPos,
          end: openPos,
          operator: "-n",
          operand
        };
        return { type: "TestGroup", pos: openPos, end, expression };
      }
      this.syntaxDepth++;
      const expr = this.parseTestOr();
      this.syntaxDepth--;
      const closeEnd = this.acceptEnd(Token.RParen, LexContext.TestMode);
      if (closeEnd < 0)
        this.error("expected ')' to close test group", this.tok.getPos());
      const end = closeEnd >= 0 ? closeEnd : openPos;
      return { type: "TestGroup", pos: openPos, end, expression: expr };
    }
    const first = this.tok.next(LexContext.TestMode);
    const val = first.value;
    const firstPos = first.pos;
    const firstEnd = first.end;
    if (first.keywordEligible && UNARY_TEST_OPS[val] === 1) {
      const nt = this.tok.peek(LexContext.TestMode).token;
      if (nt === Token.Word) {
        const operand = this.readWord(LexContext.TestMode);
        return {
          type: "TestUnary",
          pos: firstPos,
          end: operand.end,
          operator: val,
          operand
        };
      }
    }
    const nt = this.tok.peek(LexContext.TestMode);
    if (nt.token === Token.Word && nt.keywordEligible && BINARY_TEST_OPS[nt.value] === 1) {
      const op = this.tok.next(LexContext.TestMode).value;
      let right;
      if (op === "=~") {
        const token = this.tok.readTestRegexWord();
        right = new WordImpl(this.source.slice(token.pos, token.end), token.pos, token.end, this.source, computeEmbeddedWordParts, this.depth);
      } else {
        right = this.readWord(LexContext.TestMode);
      }
      const left = this.toWordFromPosEnd(first, firstPos, firstEnd);
      return {
        type: "TestBinary",
        pos: firstPos,
        end: right.end,
        operator: op,
        left,
        right
      };
    }
    const w = this.toWordFromPosEnd(first, firstPos, firstEnd);
    return { type: "TestUnary", pos: firstPos, end: w.end, operator: "-n", operand: w };
  }
  functionDef() {
    const pos = this.tok.next(LexContext.CommandStart).pos;
    const name = this.readWord(LexContext.Normal);
    let body;
    if (this.tok.peek(LexContext.CommandStart).token === Token.LParen) {
      const openPos = this.tok.next(LexContext.CommandStart).pos;
      if (this.tok.peek(LexContext.CommandStart).token === Token.RParen) {
        this.tok.next(LexContext.CommandStart);
        this.skipNewlines(LexContext.CommandStart);
        body = this.commandAsBody();
      } else {
        body = this.subshellBody(openPos);
      }
    } else {
      this.skipNewlines(LexContext.CommandStart);
      body = this.commandAsBody();
    }
    const redirects = this._redirects;
    this._redirects = EMPTY_REDIRECTS;
    const end = redirects.length > 0 ? redirects[redirects.length - 1].end : body.end;
    return { type: "Function", pos, end, name, body, redirects: ownEmpty(redirects) };
  }
  simpleCommandOrFunction() {
    const prefix = [];
    let redirects = [];
    let cmdPos = this.tok.peek(LexContext.CommandStart).pos;
    let lastEnd = cmdPos;
    let ctx = LexContext.CommandStart;
    for (;; ) {
      const t = this.tok.peek(ctx).token;
      if (t === Token.Assignment) {
        const assignment = this.tok.next(ctx);
        lastEnd = assignment.end;
        prefix.push(this.parseAssignment(assignment));
      } else if (t === Token.Redirect) {
        redirects = this.collectRedirect(redirects, ctx);
        lastEnd = redirects[redirects.length - 1].end;
      } else {
        break;
      }
      ctx = LexContext.CommandPrefix;
    }
    if (this.tok.peek(LexContext.Normal).token !== Token.Word) {
      return {
        type: "Command",
        pos: cmdPos,
        end: lastEnd,
        name: undefined,
        prefix,
        suffix: [],
        redirects
      };
    }
    const name = this.readWord(LexContext.Normal);
    lastEnd = name.end;
    if (this.tok.peek(LexContext.Normal).token === Token.LParen) {
      this.tok.next(LexContext.Normal);
      if (this.tok.peek(LexContext.Normal).token === Token.RParen) {
        this.tok.next(LexContext.Normal);
        this.skipNewlines(LexContext.CommandStart);
        const body = this.commandAsBody();
        const bodyRedirects = this._redirects;
        this._redirects = EMPTY_REDIRECTS;
        const end = bodyRedirects.length > 0 ? bodyRedirects[bodyRedirects.length - 1].end : body.end;
        return {
          type: "Function",
          pos: name.pos,
          end,
          name,
          body,
          redirects: ownEmpty(bodyRedirects)
        };
      }
    }
    const suffix = [];
    for (;; ) {
      const st = this.tok.peek(LexContext.Normal).token;
      if (st === Token.Word || st === Token.Assignment) {
        const w = this.readWord(LexContext.Normal);
        suffix.push(w);
        lastEnd = w.end;
      } else if (st === Token.Redirect) {
        redirects = this.collectRedirect(redirects, LexContext.Normal);
        lastEnd = redirects[redirects.length - 1].end;
      } else {
        break;
      }
    }
    return {
      type: "Command",
      pos: cmdPos,
      end: lastEnd,
      name,
      prefix,
      suffix,
      redirects
    };
  }
  collectRedirect(redirects, ctx) {
    if (redirects === EMPTY_REDIRECTS)
      redirects = [];
    const t = this.tok.next(ctx);
    const tPos = t.pos;
    const tEnd = t.end;
    const r = {
      pos: tPos,
      end: tEnd,
      operator: REDIRECT_OPS[t.value] ?? ">",
      target: undefined,
      fileDescriptor: t.fileDescriptor,
      variableName: t.variableName,
      content: t.content,
      heredocQuoted: undefined,
      body: undefined
    };
    if (t.targetEnd > t.targetPos) {
      const heredoc = t.value === "<<" || t.value === "<<-";
      const resolver = heredoc ? heredocDelimiterParts(t.content ?? "") : undefined;
      const text = this.source.slice(t.targetPos, t.targetEnd);
      r.target = new WordImpl(text, t.targetPos, t.targetEnd, this.source, resolver, this.depth);
    } else {
      this.error("expected redirect target", t.targetPos);
    }
    if (r.target && (t.value === "<<" || t.value === "<<-"))
      this.tok.registerHereDocTarget(r);
    redirects.push(r);
    return redirects;
  }
  commandAsBody() {
    const t = this.tok.peek(LexContext.CommandStart).token;
    if (t === Token.LBrace)
      return this.braceGroup();
    if (t === Token.LParen)
      return this.subshell();
    const cmd = this.command();
    const p = this.tok.getPos();
    return cmd ?? { type: "CompoundList", pos: p, end: p, commands: [] };
  }
  readWord(ctx) {
    return this.toWord(this.tok.next(ctx));
  }
  toWord(tok) {
    const text = tok.raw ? tok.value : this.source.slice(tok.pos, tok.end);
    return new WordImpl(text, tok.pos, tok.end, this.source, undefined, this.depth);
  }
  toWordFromPosEnd(tok, pos, end) {
    const text = tok.raw && tok.pos === pos && tok.end === end ? tok.value : this.source.slice(pos, end);
    return new WordImpl(text, pos, end, this.source, undefined, this.depth);
  }
  parseAssignment(tok) {
    const text = tok.raw ? tok.value : this.source.slice(tok.pos, tok.end);
    const tokPos = tok.pos;
    const tokEnd = tok.end;
    const result = {
      type: "Assignment",
      pos: tokPos,
      end: tokEnd,
      text,
      name: undefined,
      value: undefined,
      append: undefined,
      index: undefined,
      indexParts: undefined,
      array: undefined
    };
    const eqIdx = tok.assignmentOperatorPos - tokPos;
    if (eqIdx <= 0)
      return result;
    let nameEnd = eqIdx;
    let append = false;
    let index;
    let appendPos = eqIdx;
    while (appendPos >= 2 && text.charCodeAt(appendPos - 2) === 92 && text.charCodeAt(appendPos - 1) === 10)
      appendPos -= 2;
    if (text.charCodeAt(appendPos - 1) === 43) {
      append = true;
      nameEnd = appendPos - 1;
    }
    const bracketIdx = text.indexOf("[");
    if (bracketIdx > 0 && bracketIdx < nameEnd) {
      const rbracketIdx = text.lastIndexOf("]", eqIdx);
      if (rbracketIdx > bracketIdx) {
        index = text.slice(bracketIdx + 1, rbracketIdx);
        nameEnd = bracketIdx;
      }
    }
    const rawName = text.slice(0, nameEnd);
    const name = rawName.includes("\\\n") ? rawName.split("\\\n").join("") : rawName;
    result.name = name;
    if (append)
      result.append = true;
    if (index !== undefined) {
      result.index = index;
      const indexPos = tokPos + bracketIdx + 1;
      const indexEnd = indexPos + index.length;
      if (hasEmbeddedWordStructure(this.source, indexPos, indexEnd)) {
        const indexWord = new WordImpl(index, indexPos, indexEnd, this.source, computeEmbeddedWordParts, this.depth);
        Object.defineProperty(result, "indexParts", {
          configurable: true,
          enumerable: true,
          get: () => indexWord.parts,
          set: (value) => {
            indexWord.parts = value;
          }
        });
      }
    }
    const valStart = eqIdx + 1;
    const valueStart = tokPos + valStart;
    if (valStart < text.length && text.charCodeAt(valStart) === 40 && text.charCodeAt(text.length - 1) === 41) {
      const elements = this.parseArrayElements(valueStart + 1, tokEnd - 1);
      result.array = elements;
    } else {
      result.value = new WordImpl(text.slice(valStart), valueStart, tokEnd, this.source, undefined, this.depth);
    }
    return result;
  }
  parseArrayElements(start, end) {
    const subTok = new Lexer(this.source, start, end);
    const elements = [];
    while (subTok.peek(LexContext.Normal).token !== Token.EOF) {
      if (subTok.peek(LexContext.Normal).token === Token.Newline) {
        subTok.next(LexContext.Normal);
        continue;
      }
      const t = subTok.next(LexContext.Normal);
      if (t.token === Token.Word || t.token === Token.Assignment) {
        const text = t.raw ? t.value : this.source.slice(t.pos, t.end);
        elements.push(new WordImpl(text, t.pos, t.end, this.source, undefined, this.depth));
      }
    }
    return elements;
  }
  makeCompoundList(commands) {
    const p = this.tok.getPos();
    const pos = commands.length > 0 ? commands[0].pos : p;
    const end = commands.length > 0 ? commands[commands.length - 1].end : p;
    return { type: "CompoundList", pos, end, commands };
  }
}

// packages/toolu-core/src/shell/shell-argv.ts
import { basename } from "path";

// packages/toolu-core/src/shell/shell-options.ts
function parseArgs(words, start, spec) {
  const options = [];
  const operandAt = [];
  let missingValue = false;
  let i = start;
  const option = spec.plus === true ? /^[-+]./ : /^-/;
  const takeNext = (name) => {
    i += 1;
    missingValue ||= i >= words.length;
    options.push({ name, value: words[i], at: i });
  };
  const done = (next) => ({
    options,
    operands: operandAt.map((at) => words[at] ?? null),
    operandAt,
    next,
    missingValue
  });
  for (;i < words.length; i++) {
    const word = words[i] ?? null;
    if (word === null || word === "-" || word === "--" || !option.test(word)) {
      if (spec.stopAtOperand === true)
        return done(word === "--" ? i + 1 : i);
      if (word === "--") {
        for (let rest = i + 1;rest < words.length; rest++)
          operandAt.push(rest);
        return done(words.length);
      }
      operandAt.push(i);
    } else if (word.startsWith("--")) {
      const [name = "", value] = word.slice(2).split(/=(.*)/s);
      if (value !== undefined)
        options.push({ name, value, at: null });
      else if (named(spec.valueLong ?? "", name))
        takeNext(name);
      else
        options.push({ name, value: undefined, at: null });
    } else if (spec.numeric === true && /^-\d+$/.test(word)) {
      options.push({ name: word.slice(1), value: undefined, at: null });
    } else {
      for (let j = 1;j < word.length; j++) {
        const name = word.charAt(j);
        const rest = word.slice(j + 1);
        const valued = spec.valueShort?.includes(name) === true;
        if (spec.restShort?.includes(name) === true || valued && rest !== "") {
          options.push({ name, value: rest, at: null });
          break;
        }
        if (valued) {
          takeNext(name);
          break;
        }
        options.push({ name, value: undefined, at: null });
      }
    }
  }
  return done(words.length);
}
function named(names, name) {
  return ` ${names} `.includes(` ${name} `);
}
function optionValues(parsed, names) {
  return parsed.options.filter((option) => named(names, option.name)).map((o) => o.value);
}
function hasOption(parsed, names) {
  return parsed.options.some((option) => named(names, option.name));
}

// packages/toolu-core/src/shell/shell-argv.ts
var WRAPPERS = {
  sudo: {
    valueShort: "ugCDprtTU",
    valueLong: "user group close-from chdir prompt role type",
    inert: "e l v K V h edit list validate remove-timestamp version help",
    assignments: true
  },
  doas: { valueShort: "uC", inert: "L" },
  env: {
    valueShort: "uCPa",
    valueLong: "unset chdir argv0",
    opaque: "S split-string",
    assignments: true,
    dashOption: true
  },
  command: { inert: "v V" },
  builtin: {},
  exec: { valueShort: "a" },
  nohup: {},
  time: { valueShort: "fo", valueLong: "format output" },
  nice: { valueShort: "n", valueLong: "adjustment", numeric: true },
  timeout: { valueShort: "sk", valueLong: "signal kill-after", operands: 1 },
  xargs: {
    valueShort: "adEILnPs",
    valueLong: "arg-file delimiter max-args max-procs max-chars process-slot-var",
    appendsDynamic: true
  },
  stdbuf: { valueShort: "ioe", valueLong: "input output error" }
};
var ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/;
function wrapperOf(name) {
  if (name === null || name === undefined)
    return;
  return Object.hasOwn(WRAPPERS, basename(name)) ? WRAPPERS[basename(name)] : undefined;
}
function innerStart(words, wrapper) {
  const parsed = parseArgs(words, 1, { ...wrapper, stopAtOperand: true });
  if (parsed.missingValue || hasOption(parsed, wrapper.inert ?? ""))
    return null;
  if (hasOption(parsed, wrapper.opaque ?? ""))
    return "opaque";
  let start = parsed.next;
  for (;start < words.length; start++) {
    const word = words[start] ?? "";
    const dash = wrapper.dashOption === true && word === "-";
    if (!dash && !(wrapper.assignments === true && ASSIGNMENT.test(word)))
      break;
  }
  start += wrapper.operands ?? 0;
  return start < words.length ? start : null;
}
function unwrap(words) {
  let start = 0;
  let appendsDynamic = false;
  const wrappers = [];
  for (let wrapper = wrapperOf(words[0]);wrapper !== undefined; wrapper = wrapperOf(words[start])) {
    const inner = innerStart(words.slice(start), wrapper);
    if (inner === null)
      break;
    wrappers.push(basename(words[start] ?? ""));
    if (inner === "opaque")
      return { wrappers, start: null, appendsDynamic };
    appendsDynamic ||= wrapper.appendsDynamic === true;
    start += inner;
  }
  return { wrappers, start, appendsDynamic };
}
function alignUnwrapped(list, unwrapped, fill) {
  if (unwrapped.start === null)
    return [fill];
  return [...list.slice(unwrapped.start), ...unwrapped.appendsDynamic ? [fill] : []];
}
var SHELLS = new Set(["bash", "sh", "zsh", "dash", "ksh"]);
var SHELL_OPTIONS = {
  valueShort: "oO",
  valueLong: "rcfile init-file",
  stopAtOperand: true,
  plus: true
};
function shellTarget(argv) {
  const parsed = parseArgs(argv, 1, SHELL_OPTIONS);
  const at = argv[parsed.next] === "-" ? parsed.next + 1 : parsed.next;
  const operand = argv[at];
  if (hasOption(parsed, "c"))
    return at < argv.length ? { origin: "shell", script: operand ?? null, stdin: false } : null;
  if (hasOption(parsed, "s") || at >= argv.length)
    return { origin: "shell", script: null, stdin: true };
  return operand === null ? { origin: "shell", script: null, stdin: false } : null;
}
function runTarget(argv) {
  const name = argv[0];
  if (name === null || name === undefined)
    return null;
  if (SHELLS.has(basename(name)))
    return shellTarget(argv);
  if (name !== "eval")
    return null;
  const args = argv[1] === "--" ? argv.slice(2) : argv.slice(1);
  if (args.length === 0)
    return null;
  const script = args.some((arg) => arg === null) ? null : args.join(" ");
  return { origin: "eval", script, stdin: false };
}

// packages/toolu-core/src/shell/shell-types.ts
function unreachable(value) {
  throw new Error(`@toolu/core/shell: unhandled node ${JSON.stringify(value)}`);
}

// packages/toolu-core/src/shell/shell-words.ts
var IGNORE = () => {
  return;
};
function heredocContent(redirect) {
  const content = redirect.content ?? "";
  if (redirect.heredocQuoted !== true && /[$`\\]/.test(content))
    return null;
  return redirect.operator === "<<-" ? content.replace(/^\t+/gm, "") : content;
}
function isHeredoc(redirect) {
  return redirect.operator === "<<" || redirect.operator === "<<-";
}
function heredocCat(script) {
  if (script === undefined || (script.errors?.length ?? 0) > 0)
    return null;
  const [statement, ...others] = script.commands;
  if (statement === undefined || others.length > 0 || statement.background === true)
    return null;
  const command = statement.command;
  if (command.type !== "Command" || statement.redirects.length > 0)
    return null;
  if (command.prefix.length > 0 || command.suffix.length > 0)
    return null;
  if (scanWord(command.name, IGNORE).value !== "cat")
    return null;
  const [redirect, ...more] = command.redirects;
  if (redirect === undefined || more.length > 0 || !isHeredoc(redirect))
    return null;
  return heredocContent(redirect)?.replace(/\n+$/, "") ?? null;
}
function scanPart(part, visit, quoted) {
  switch (part.type) {
    case "Literal":
    case "SingleQuoted":
    case "AnsiCQuoted":
      return part.value;
    case "DoubleQuoted":
    case "LocaleString":
      return scanParts(part.parts, visit, true);
    case "SimpleExpansion":
      return null;
    case "ParameterExpansion":
      scanParts(part.indexParts, visit, false);
      for (const word of [part.operand, part.slice?.offset, part.slice?.length])
        scanWord(word, visit);
      for (const word of [part.replace?.pattern, part.replace?.replacement])
        scanWord(word, visit);
      return null;
    case "CommandExpansion":
      visit(part.script, part.text);
      return quoted ? heredocCat(part.script) : null;
    case "ProcessSubstitution":
      visit(part.script, part.text);
      return null;
    case "ArithmeticExpansion":
      visitArithmetic(part.expression, visit);
      return null;
    case "ExtendedGlob":
    case "BraceExpansion":
      scanParts(part.parts, visit, false);
      return part.type === "ExtendedGlob" ? part.text : null;
    default:
      return unreachable(part);
  }
}
function scanParts(parts, visit, quoted) {
  let value = "";
  for (const part of parts ?? []) {
    const piece = scanPart(part, visit, quoted);
    value = value === null || piece === null ? null : value + piece;
  }
  return value;
}
function hasGlob(raw) {
  const unescaped = raw.replace(/\\./gs, "");
  return /[*?]/.test(unescaped) || /\[[^\]]*\]/.test(unescaped);
}
function scanWord(word, visit) {
  if (word === undefined)
    return { value: null, pattern: null, text: "" };
  const { parts, value: text } = word;
  const value = parts === undefined ? text : scanParts(parts, visit, false);
  const glob = parts === undefined ? hasGlob(word.text) : parts.some((p) => p.type === "ExtendedGlob" || p.type === "Literal" && hasGlob(p.text));
  return glob && value !== null ? { value: null, pattern: value, text } : { value, pattern: null, text };
}
function visitArithmetic(expression, visit) {
  if (expression === undefined)
    return;
  switch (expression.type) {
    case "ArithmeticBinary":
      visitArithmetic(expression.left, visit);
      visitArithmetic(expression.right, visit);
      break;
    case "ArithmeticUnary":
      visitArithmetic(expression.operand, visit);
      break;
    case "ArithmeticTernary":
      visitArithmetic(expression.test, visit);
      visitArithmetic(expression.consequent, visit);
      visitArithmetic(expression.alternate, visit);
      break;
    case "ArithmeticGroup":
      visitArithmetic(expression.expression, visit);
      break;
    case "ArithmeticWord":
      scanParts(expression.parts, visit, false);
      break;
    case "ArithmeticCommandExpansion":
      visit(expression.script, expression.text);
      break;
    default:
      unreachable(expression);
  }
}
function visitTest(expression, visit) {
  switch (expression.type) {
    case "TestUnary":
      scanWord(expression.operand, visit);
      break;
    case "TestBinary":
      scanWord(expression.left, visit);
      scanWord(expression.right, visit);
      break;
    case "TestLogical":
      visitTest(expression.left, visit);
      visitTest(expression.right, visit);
      break;
    case "TestNot":
      visitTest(expression.operand, visit);
      break;
    case "TestGroup":
      visitTest(expression.expression, visit);
      break;
    default:
      unreachable(expression);
  }
}
function visitAssignment(assignment, visit) {
  scanParts(assignment.indexParts, visit, false);
  scanWord(assignment.value, visit);
  for (const word of assignment.array ?? [])
    scanWord(word, visit);
}
function toShellRedirect(redirect, visit) {
  const heredoc = isHeredoc(redirect);
  const target = scanWord(redirect.target, visit);
  if (redirect.heredocQuoted !== true)
    scanWord(redirect.body, visit);
  return {
    operator: redirect.operator,
    fd: redirect.fileDescriptor ?? null,
    target: heredoc ? null : target.value,
    pattern: heredoc ? null : target.pattern,
    text: heredoc ? "" : target.text,
    heredoc: heredoc ? { content: heredocContent(redirect), quoted: redirect.heredocQuoted === true } : null
  };
}
function stdinScript(redirects) {
  for (const redirect of redirects) {
    if (redirect.fd !== null && redirect.fd !== 0)
      continue;
    if (redirect.heredoc !== null)
      return redirect.heredoc.content;
    if (redirect.operator === "<<<")
      return redirect.target;
  }
  return null;
}

// packages/toolu-core/src/shell/shell-walk.ts
var MAX_RUN_DEPTH = 4;
var ALONE = { index: 0, size: 1 };
function unknownCommand(text, origin, depth, sink) {
  sink.commands.push({
    words: [null],
    argv: [null],
    patterns: [null],
    texts: [text],
    wrappers: [],
    redirects: [],
    pipeline: ALONE,
    exitProves: false,
    origin,
    depth,
    text
  });
}
function nestedVisitor(ctx, sink) {
  return (script, text) => {
    if (script === undefined) {
      unknownCommand(text, "substitution", ctx.depth, sink);
      return;
    }
    const source = script.source ?? ctx.source;
    walkScript(script, { source, origin: "substitution", depth: ctx.depth, proves: false, pipeline: ALONE }, sink);
  };
}
function compoundRedirects(redirects, ctx, sink) {
  for (const redirect of redirects) {
    sink.compoundRedirects.push(toShellRedirect(redirect, nestedVisitor(ctx, sink)));
  }
}
function runString(target, redirects, text, ctx, sink) {
  const script = target.stdin ? stdinScript(redirects) : target.script;
  if (script === null || ctx.depth + 1 > MAX_RUN_DEPTH) {
    unknownCommand(text, target.origin, ctx.depth + 1, sink);
    return;
  }
  const inner = { ...ctx, source: script, origin: target.origin, depth: ctx.depth + 1 };
  walkScript(parse4(script), inner, sink);
}
function emitCommand(command, ctx, sink) {
  const nested = nestedVisitor(ctx, sink);
  for (const assignment of command.prefix)
    visitAssignment(assignment, nested);
  const named = command.name === undefined ? [] : [command.name, ...command.suffix];
  const resolved = named.map((word) => scanWord(word, nested));
  const redirects = command.redirects.map((redirect) => toShellRedirect(redirect, nested));
  const words = resolved.map((word) => word.value);
  const unwrapped = unwrap(words);
  const argv = alignUnwrapped(words, unwrapped, null);
  const text = ctx.source.slice(command.pos, command.end);
  const proves = ctx.proves && !unwrapped.wrappers.includes("xargs");
  sink.commands.push({
    words,
    argv,
    patterns: alignUnwrapped(resolved.map((word) => word.pattern), unwrapped, null),
    texts: alignUnwrapped(resolved.map((word) => word.text), unwrapped, ""),
    wrappers: unwrapped.wrappers,
    redirects,
    pipeline: ctx.pipeline,
    exitProves: proves,
    origin: ctx.origin,
    depth: ctx.depth,
    text
  });
  const target = runTarget(argv);
  if (target !== null)
    runString(target, redirects, text, { ...ctx, proves }, sink);
}
function walkPipeline(node, ctx, sink) {
  const size = node.commands.length;
  node.commands.forEach((command, index) => {
    const proves = ctx.proves && node.negated !== true && index === size - 1;
    walkNode(command, { ...ctx, proves, pipeline: size > 1 ? { index, size } : ctx.pipeline }, sink);
  });
}
function walkAndOr(node, ctx, sink) {
  node.commands.forEach((command, index) => {
    const before = index === 0 ? "&&" : node.operators[index - 1];
    const after = node.operators.slice(index);
    const proves = ctx.proves && before === "&&" && after.every((op) => op === "&&");
    walkNode(command, { ...ctx, proves }, sink);
  });
}
function walkList(statements, ctx, sink) {
  const last = statements.length - 1;
  statements.forEach((statement, index) => {
    const proves = ctx.proves && index === last && statement.background !== true;
    compoundRedirects(statement.redirects, ctx, sink);
    walkNode(statement.command, { ...ctx, proves }, sink);
  });
}
function walkNode(node, ctx, sink) {
  const nested = nestedVisitor(ctx, sink);
  const off = { ...ctx, proves: false };
  const list = (statements, at = off) => walkList(statements, at, sink);
  switch (node.type) {
    case "Command":
      emitCommand(node, ctx, sink);
      break;
    case "Pipeline":
      walkPipeline(node, ctx, sink);
      break;
    case "AndOr":
      walkAndOr(node, ctx, sink);
      break;
    case "If":
      for (const part of [node.clause, node.then])
        list(part.commands);
      if (node.else !== undefined)
        walkNode(node.else, off, sink);
      break;
    case "For":
    case "Select":
      for (const word of node.wordlist)
        scanWord(word, nested);
      list(node.body.commands);
      break;
    case "ArithmeticFor":
      for (const part of [node.initialize, node.test, node.update])
        visitArithmetic(part, nested);
      list(node.body.commands);
      break;
    case "While":
      for (const part of [node.clause, node.body])
        list(part.commands);
      break;
    case "Case":
      scanWord(node.word, nested);
      for (const item of node.items) {
        for (const pattern of item.pattern)
          scanWord(pattern, nested);
        list(item.body.commands);
      }
      break;
    case "Function":
    case "Coproc":
      compoundRedirects(node.redirects, ctx, sink);
      walkNode(node.body, node.type === "Function" ? { ...off, origin: "function" } : off, sink);
      break;
    case "Subshell":
    case "BraceGroup":
    case "CompoundList":
      list(node.type === "CompoundList" ? node.commands : node.body.commands, ctx);
      break;
    case "TestCommand":
      visitTest(node.expression, nested);
      break;
    case "ArithmeticCommand":
      visitArithmetic(node.expression, nested);
      break;
    case "Statement":
      list([node], ctx);
      break;
    default:
      unreachable(node);
  }
}
function walkScript(script, ctx, sink) {
  for (const error of script.errors ?? []) {
    sink.errors.push({ message: error.message, pos: error.pos, origin: ctx.origin });
  }
  walkList(script.commands, ctx, sink);
}

// packages/toolu-core/src/shell/shell-parse.ts
var MAX_SHELL_INPUT = 1024 * 1024;
function unknownAnalysis(source, error) {
  return { source, commands: [], compoundRedirects: [], errors: [error], unknown: true };
}
function analyzeShell(source) {
  if (source.length > MAX_SHELL_INPUT) {
    return unknownAnalysis(source, {
      message: `oversize: ${source.length} characters exceeds the ${MAX_SHELL_INPUT} cap`,
      pos: MAX_SHELL_INPUT,
      origin: "line"
    });
  }
  const sink = { commands: [], compoundRedirects: [], errors: [] };
  const root = {
    source,
    origin: "line",
    depth: 0,
    proves: true,
    pipeline: { index: 0, size: 1 }
  };
  try {
    walkScript(parse4(source), root, sink);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return unknownAnalysis(source, { message: `parser: ${message}`, pos: 0, origin: "line" });
  }
  if (sink.errors.length === 0)
    return { source, ...sink, unknown: false };
  const commands = sink.commands.map((command) => ({ ...command, exitProves: false }));
  return { ...sink, source, commands, unknown: commands.length === 0 };
}

// packages/toolu-core/src/shell/shell-event.ts
var analyses = new WeakMap;
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

// packages/toolu-core/src/detect/detect-git.ts
import { spawnSync as spawnSync3 } from "child_process";

// packages/toolu-core/src/shell/shell-git.ts
import { basename as basename2 } from "path";
var GLOBALS = {
  valueShort: "Cc",
  valueLong: "git-dir work-tree namespace super-prefix config-env attr-source",
  stopAtOperand: true
};
function gitInvocation(command) {
  const name = command.argv[0];
  if (name === null || name === undefined || basename2(name) !== "git")
    return;
  const globals = parseArgs(command.argv, 1, GLOBALS);
  if (globals.missingValue || globals.next >= command.argv.length)
    return;
  const cChain = optionValues(globals, "C").map((value) => value ?? null);
  const subcommand = command.argv[globals.next] ?? null;
  return { command, subcommand, args: command.argv.slice(globals.next + 1), cChain };
}
function runsGitSubcommand(analysis, sub) {
  let unknown = analysis.unknown;
  for (const command of analysis.commands) {
    if (command.argv[0] === null) {
      unknown = true;
      continue;
    }
    const git = gitInvocation(command);
    if (git?.subcommand === sub)
      return "yes";
    if (git?.subcommand === null)
      unknown = true;
  }
  return unknown ? "unknown" : "no";
}
var PUSH_OPTIONS = {
  valueShort: "o",
  valueLong: "push-option receive-pack exec repo"
};
function destinationOf(refspec) {
  if (refspec === null || refspec === undefined)
    return null;
  const spec = refspec.startsWith("+") ? refspec.slice(1) : refspec;
  if (spec.startsWith(":") || spec === "HEAD")
    return null;
  const colon = spec.indexOf(":");
  const dst = (colon === -1 ? spec : spec.slice(colon + 1)).replace(/^refs\/heads\//, "");
  return dst === "" || dst.includes("*") ? null : dst;
}
function pushTargets(analysis) {
  return analysis.commands.flatMap((command) => {
    const invocation = gitInvocation(command);
    if (invocation?.subcommand !== "push")
      return [];
    const refspec = parseArgs(invocation.args, 0, PUSH_OPTIONS).operands[1];
    const destination = destinationOf(refspec);
    return [{ invocation, cChain: invocation.cChain, refspec, destination }];
  });
}

// packages/toolu-core/src/detect/detect-git.ts
function isGitPush(analysis) {
  return runsGitSubcommand(analysis, "push") === "yes";
}
function gitOut(cwd, args, env) {
  const res = spawnSync3("git", [...args], {
    cwd: cwd ?? process.cwd(),
    env: childEnv(env),
    encoding: "utf8"
  });
  const out = res.error === undefined && res.status === 0 ? res.stdout.trim() : "";
  return out === "" ? undefined : out;
}
function pushTargetRoot(analysis, options = {}) {
  const env = options.env ?? process.env;
  const cwd = options.cwd ?? process.cwd();
  const chain = pushTargets(analysis)[0]?.cChain ?? [];
  const dirs = chain.filter((dir) => dir !== null);
  const viaChain = chain.length > 0 && dirs.length === chain.length ? gitOut(cwd, [...dirs.flatMap((dir) => ["-C", dir]), "rev-parse", "--show-toplevel"], env) : undefined;
  return viaChain ?? gitOut(cwd, ["rev-parse", "--show-toplevel"], env) ?? projectRoot({ env, cwd }) ?? cwd;
}
// packages/toolu-core/src/detect/detect-tools.ts
var cache = new Map;
// packages/toolu-core/src/gates/quality-command.ts
import { basename as basename3 } from "path";
var BUN_SCRIPTS = new Set([
  "check",
  "check:fix",
  "check:duplication",
  "ts:check",
  "ts:check:fix",
  "rust:check",
  "rust:test",
  "check-types",
  "lint",
  "lint:fix",
  "format",
  "format:check",
  "format:fix",
  "build",
  "test"
]);
var CARGO = new Set(["clippy", "test", "build", "nextest"]);
var JS_TOOLS = new Set(["vitest", "jest", "tsc"]);
var TS_CHECK = new Set(["./scripts/ts-check.sh", "scripts/ts-check.sh"]);
var WRAPPER_SCRIPT = /(?:^|\/)(tools\/[A-Za-z0-9_.-]+\/(?:check|test|format)\.sh)$/;
var PACKAGE_RUNNERS = new Set(["npx", "bunx", "pnpx", "yarn"]);
var SCRIPT_SHELLS = new Set(["bash", "sh"]);
function bunLabel(argv) {
  const [, verb, script] = argv;
  if (verb === "test")
    return "bun test";
  return verb === "run" && typeof script === "string" && BUN_SCRIPTS.has(script) ? `bun run ${script}` : undefined;
}
function cargoLabel(argv) {
  const toolchain = argv[1]?.startsWith("+") === true;
  const sub = argv[toolchain ? 2 : 1];
  return typeof sub === "string" && CARGO.has(sub) ? `cargo ${sub}` : undefined;
}
function directLabel(argv) {
  const name = argv[0];
  if (typeof name !== "string")
    return;
  const script = WRAPPER_SCRIPT.exec(name)?.[1];
  if (script !== undefined)
    return script;
  if (TS_CHECK.has(name))
    return name;
  const base = basename3(name);
  if (JS_TOOLS.has(base))
    return base;
  if (base === "bun")
    return bunLabel(argv);
  return base === "cargo" ? cargoLabel(argv) : undefined;
}
function afterOptions(argv, from) {
  const at = argv.findIndex((word, i) => i >= from && !(word?.startsWith("-") ?? false));
  return at === -1 ? [] : argv.slice(at);
}
function runnerTarget(argv) {
  const name = argv[0];
  if (typeof name !== "string")
    return;
  const base = basename3(name);
  if (PACKAGE_RUNNERS.has(base))
    return afterOptions(argv, 1);
  if (base === "bun" && argv[1] === "x" || base === "pnpm" && argv[1] === "exec") {
    return afterOptions(argv, 2);
  }
  const script = argv[1];
  return SCRIPT_SHELLS.has(base) && typeof script === "string" && !script.startsWith("-") ? argv.slice(1) : undefined;
}
function labelOf(command) {
  if (command.origin === "function")
    return;
  const direct = directLabel(command.argv);
  if (direct !== undefined)
    return direct;
  const target = runnerTarget(command.argv);
  return target === undefined ? undefined : directLabel(target);
}
function qualityCommands(analysis) {
  return analysis.commands.flatMap((command) => {
    const label = labelOf(command);
    return label === undefined ? [] : [{ command, label }];
  });
}
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
function isObject2(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function get(value, key) {
  if (value === null)
    return null;
  if (isObject2(value) && typeof key === "string") {
    return Object.hasOwn(value, key) ? value[key] ?? null : null;
  }
  throw new JqError(`Cannot index ${jqType(value)} with ${jqType(key)}`);
}
function alt(value, fallback) {
  return value === undefined || value === null || value === false ? fallback : value;
}
function raw(value) {
  return typeof value === "string" ? value : toJqJson(value, true);
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

// packages/toolu-core/src/gates/tool-exit.ts
function asJson(payload) {
  return parseJson(JSON.stringify(payload)) ?? null;
}
function firstOf(doc, paths, fallback) {
  try {
    for (const path of paths) {
      const value = path.reduce((at, key) => get(at, key), doc);
      if (alt(value, null) !== null)
        return value;
    }
    return fallback;
  } catch (error) {
    if (error instanceof JqError)
      return;
    throw error;
  }
}
function printed2(value, onError) {
  if (value === undefined)
    return onError;
  if (value === null)
    return "";
  return raw(value).replace(/\n+$/, "");
}
function toolCommand(payload) {
  return printed2(firstOf(asJson(payload), [["tool_input", "command"]], ""), "");
}
var EXIT_PATHS = [
  ["tool_response", "metadata", "exit_code"],
  ["tool_response", "exit_code"],
  ["tool_output", "exit_code"]
];
function toolExitStatus(payload) {
  const doc = asJson(payload);
  const status = printed2(firstOf(doc, EXIT_PATHS, null), "");
  if (status !== "" && status !== "null")
    return status;
  const output = printed2(firstOf(doc, [["tool_output"]], null), "");
  if (output === "")
    return status;
  const inner = parseJson(output);
  if (inner === undefined)
    return "";
  return printed2(firstOf(inner, [["exitCode"], ["exit_code"]], null), "");
}
function toolInterrupted(payload) {
  return printed2(firstOf(asJson(payload), [["tool_response", "interrupted"]], false), "false") === "true";
}
// packages/toolu-core/src/gates/gate-status.ts
import { existsSync as existsSync2, mkdirSync as mkdirSync3, writeFileSync as writeFileSync4 } from "fs";
import { join as join8 } from "path";

// packages/toolu-core/src/state/gate-file.ts
import { appendFileSync as appendFileSync2, existsSync, readFileSync as readFileSync5, writeFileSync as writeFileSync3 } from "fs";
import { dirname as dirname2 } from "path";

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
var text3 = string2();
var maybeText = string2().nullable();
var TELEMETRY_EXTRAS = {
  gate_fail: strictObject({ file: text3, source: text3 }),
  gate_clear: strictObject({ file: text3, source: text3 }),
  step_run: strictObject({
    step_id: text3,
    status: text3,
    exit_code: number2(),
    duration_s: number2(),
    attempt: number2()
  }),
  ac_coverage: strictObject({ covered: number2(), uncovered: number2() }),
  docs_attested: strictObject({ decision: text3 }),
  docs_nudge: strictObject({}),
  push_check: strictObject({ result: text3, reason_code: text3, round: number2().nullable() }),
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
  t: text3,
  branch: text3
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
import { appendFileSync, mkdirSync as mkdirSync2 } from "fs";
import { join as join7 } from "path";

// packages/toolu-core/src/state/state-git.ts
import { spawnSync as spawnSync4 } from "child_process";
function currentBranch(root, env) {
  const res = spawnSync4("git", ["-C", root, "rev-parse", "--abbrev-ref", "HEAD"], {
    env: childEnv(env),
    encoding: "utf8"
  });
  return res.error === undefined ? res.stdout.replace(/\n+$/, "") : "";
}

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
  const warn = options.warn ?? stderrWarn2;
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
  const file = join7(dir, `${branchSlug(branch)}.jsonl`);
  try {
    mkdirSync2(dir, { recursive: true });
    appendFileSync(file, `${line}
`);
  } catch (error) {
    return skip(`could not append to ${file}: ${String(error)}`);
  }
  return { written: true, file };
}

// packages/toolu-core/src/state/gate-file.ts
var GLOBAL_GATE_KEY = "__global__";
function isGateFile(value) {
  return GateFileSchema.safeParse(value).success;
}
function firstIssue(value) {
  const parsed = GateFileSchema.safeParse(value);
  const issue = parsed.success ? undefined : parsed.error.issues[0];
  return issue === undefined ? "invalid" : `${issue.path.join(".") || "(root)"}: ${issue.message}`;
}
function readGateFile(gateFile) {
  if (!existsSync(gateFile))
    return { kind: "missing" };
  let value;
  try {
    value = JSON.parse(readFileSync5(gateFile, "utf8"));
  } catch (error) {
    return { kind: "malformed", reason: String(error) };
  }
  if (value === null || value === false)
    return { kind: "malformed", reason: String(value) };
  if (isGateFile(value))
    return { kind: "ok", doc: value };
  return { kind: "unrecognized", reason: firstIssue(value), value };
}
function seedEntries(doc) {
  if (doc.status !== "failing")
    return {};
  if (doc.entries !== undefined)
    return doc.entries;
  const { source, reason, violations, updatedAt } = doc;
  return { [doc.file]: { source, reason, violations, updatedAt } };
}
function sortedEntries(entries) {
  return Object.entries(entries).toSorted(([ka, a], [kb, b]) => compareJqStrings(a.updatedAt, b.updatedAt) || compareJqStrings(ka, kb));
}
function joinViolations(sorted) {
  return sorted.map(([, entry]) => entry.violations).join("");
}
function gateRoot(gateFile) {
  return dirname2(dirname2(dirname2(gateFile)));
}
function droppedCount(value, file) {
  if (!isJsonObject(value))
    return 0;
  let keys = [];
  if (isJsonObject(value.entries)) {
    keys = Object.keys(value.entries);
  } else if (value.status === "failing") {
    keys = [typeof value.file === "string" ? value.file : GLOBAL_GATE_KEY];
  }
  return keys.filter((key) => key !== file).length;
}
function breadcrumb(gateFile, line) {
  try {
    appendFileSync2(`${gateFile}.dropped.log`, `${line}
`);
  } catch {}
}
function failingDoc(prev, f) {
  const entry = { source: f.source, reason: f.reason, violations: f.violations, updatedAt: f.now };
  const entries = { ...prev, [f.file]: entry };
  return {
    status: "failing",
    reason: f.reason,
    source: f.source,
    file: f.file,
    violations: joinViolations(sortedEntries(entries)),
    entries,
    updatedAt: f.now
  };
}
function writeSingleSlot(gateFile, previous, f, warn) {
  const dropped = droppedCount(previous, f.file);
  if (dropped > 0) {
    warn(`gate-file: primary write failed at ${gateFile}; single-slot fallback dropped ${String(dropped)} other entry(ies)`);
    breadcrumb(gateFile, `${f.now} primary write failed; single-slot fallback dropped ${String(dropped)} entry(ies)`);
  }
  const { reason, source, file, violations, now } = f;
  const doc = { status: "failing", reason, source, file, violations, updatedAt: now };
  try {
    writeFileSync3(gateFile, `${toJqJson(doc, true)}
`);
  } catch {}
}
function recordGateFailure(gateFile, file, source, reason, violations, options = {}) {
  const warn = options.warn ?? stderrWarn2;
  const f = { file, source, reason, violations, now: isoSeconds(options.now?.() ?? new Date) };
  withLock(gateFile, () => {
    const existing = readGateFile(gateFile);
    let prev = {};
    let previous = {};
    if (existing.kind === "ok") {
      prev = seedEntries(existing.doc);
      previous = existing.doc;
    } else if (existing.kind === "unrecognized") {
      previous = existing.value;
      const dropped = droppedCount(existing.value, file);
      warn(`gate-file: unrecognized gate file at ${gateFile} (${existing.reason}); replacing it`);
      breadcrumb(gateFile, `${f.now} unrecognized gate file replaced; dropped ${String(dropped)} entry(ies)`);
    }
    if (!writeAtomic(gateFile, `${toJqJson(failingDoc(prev, f), true)}
`)) {
      writeSingleSlot(gateFile, previous, f, warn);
    }
  }, { warn });
  telemetryAppend(gateRoot(gateFile), "gate_fail", { file, source }, options);
}
function owns(doc, file, source) {
  if (doc.entries === undefined)
    return doc.source === source && doc.file === file;
  const entry = Object.hasOwn(doc.entries, file) ? doc.entries[file] : undefined;
  return (entry?.source ?? "") === source;
}
function clearedDoc(left, source, now) {
  const sorted = sortedEntries(left);
  const latest = sorted.at(-1);
  if (latest === undefined)
    return { status: "passing", source, updatedAt: now };
  const [key, entry] = latest;
  return {
    status: "failing",
    reason: entry.reason,
    source: entry.source,
    file: key,
    violations: joinViolations(sorted),
    entries: left,
    updatedAt: now
  };
}
function plannedClear(gateFile, file, source, now, warn) {
  const existing = readGateFile(gateFile);
  if (existing.kind === "malformed") {
    warn(`gate-file: malformed JSON at ${gateFile}; ignoring clear (gate stays failing until next write)`);
  } else if (existing.kind === "unrecognized") {
    warn(`gate-file: unrecognized gate file at ${gateFile} (${existing.reason}); ignoring clear`);
  }
  if (existing.kind !== "ok")
    return;
  const doc = existing.doc;
  if (doc.status !== "failing" || !owns(doc, file, source))
    return;
  const left = { ...seedEntries(doc) };
  delete left[file];
  return `${toJqJson(clearedDoc(left, source, now), true)}
`;
}
function clearGateFile(gateFile, file, source, options = {}) {
  const warn = options.warn ?? stderrWarn2;
  const now = isoSeconds(options.now?.() ?? new Date);
  if (plannedClear(gateFile, file, source, now, warn) === undefined)
    return "noop";
  const cleared = withLock(gateFile, () => {
    const body = plannedClear(gateFile, file, source, now, () => {});
    return body !== undefined && writeAtomic(gateFile, body);
  }, { warn });
  if (!cleared)
    return "noop";
  telemetryAppend(gateRoot(gateFile), "gate_clear", { file, source }, options);
  return "cleared";
}

// packages/toolu-core/src/gates/command-analysis.ts
var analyses2 = new WeakMap;
function isShellTool(event) {
  return event.toolName === "Bash" || event.toolName === "Shell";
}
function commandAnalysis(event, ctx) {
  const cached = analyses2.get(event);
  if (cached !== undefined)
    return cached;
  const analysis = analyzeShell(toolCommand(ctx.raw));
  analyses2.set(event, analysis);
  return analysis;
}

// packages/toolu-core/src/gates/gate-status.ts
var SOURCE = "gate-status-hook";
var ALLOW2 = { kind: "allow" };
function failingContext(command, status) {
  return `Global quality gate failing. Fix all errors/warnings/tests before new tasks.\\nFailed: ${command} (exit ${status})`;
}
function gateDir(ctx) {
  const dir = projectStateRoot({ env: ctx.env, host: ctx.host, root: ctx.projectRoot });
  if (dir === undefined)
    throw new Error("gate-status: no project state root");
  mkdirSync3(dir, { recursive: true });
  return dir;
}
function writeFirstPass(gateFile, command) {
  const doc = { status: "passing", source: command, updatedAt: isoSeconds(new Date) };
  writeFileSync4(gateFile, `${toJqJson(doc, true)}
`);
}
function decide(event, ctx) {
  if (!isShellTool(event))
    return ALLOW2;
  const gateFile = join8(gateDir(ctx), "quality-gate-status.json");
  const quality = qualityCommands(commandAnalysis(event, ctx));
  if (quality.length === 0)
    return ALLOW2;
  const command = toolCommand(ctx.raw);
  const status = toolExitStatus(ctx.raw);
  const state = { env: ctx.env, host: ctx.host };
  if (/^[0-9]+$/.test(status) && Number(status) !== 0) {
    if (!quality.some((q) => q.command.exitProves))
      return ALLOW2;
    const reason = `Quality command failed: ${command} (exit ${status})`;
    recordGateFailure(gateFile, GLOBAL_GATE_KEY, SOURCE, reason, "", state);
    return { kind: "advisory", message: failingContext(command, status) };
  }
  if (status === "0" && quality.every((q) => q.command.exitProves)) {
    clearGateFile(gateFile, GLOBAL_GATE_KEY, SOURCE, state);
    if (!existsSync2(gateFile))
      writeFirstPass(gateFile, command);
  }
  return ALLOW2;
}
var gateStatusModule = {
  kind: "native",
  name: "gate-status.sh",
  run: (event, ctx) => Promise.resolve(decide(event, ctx))
};
// packages/toolu-core/src/ledger/push-waiver.ts
import { mkdirSync as mkdirSync4, readFileSync as readFileSync6, rmSync as rmSync3, statSync as statSync6 } from "fs";
import { dirname as dirname3 } from "path";
var PUSH_WAIVER_VERSION = 1;
function pushWaiverDir(root, options = {}) {
  const o = resolveHost({
    env: options.env ?? process.env,
    ...options.host === undefined ? {} : { host: options.host }
  });
  const override = envValue(o.env, "STATE_DIR");
  if (override !== undefined)
    return override;
  return projectStateDir("push-review", root === "" ? o : { ...o, root }) ?? "";
}
function pushWaiverPath(root, slug, options = {}) {
  return `${pushWaiverDir(root, options)}/${slug}.waiver.json`;
}
function pushWaiverPendingPath(root, slug, options = {}) {
  return `${pushWaiverDir(root, options)}/${slug}.pending-waiver.json`;
}
function readObject(file) {
  try {
    if (!statSync6(file).isFile())
      return;
    const value = parseJson(readFileSync6(file, "utf8"));
    return isObject2(value) ? value : undefined;
  } catch {
    return;
  }
}
function waiverSha(file) {
  const doc = readObject(file);
  if (doc === undefined)
    return;
  try {
    if (raw(alt(get(doc, "version"), "")) !== String(PUSH_WAIVER_VERSION))
      return;
    const sha = raw(alt(get(doc, "diff_sha"), ""));
    return sha === "" ? undefined : sha;
  } catch (error) {
    if (error instanceof JqError)
      return;
    throw error;
  }
}
function write(file, doc) {
  try {
    mkdirSync4(dirname3(file), { recursive: true });
  } catch {
    return false;
  }
  return writeAtomic(file, `${toJqJson(doc, false)}
`);
}
function now(options) {
  return isoSeconds(options.now?.() ?? new Date);
}
function pushWaiverPromote(root, slug, sha, options = {}) {
  if (sha === "")
    return false;
  const pending = pushWaiverPendingPath(root, slug, options);
  if (waiverSha(pending) !== sha)
    return false;
  const marker = readObject(pending);
  if (marker === undefined)
    return false;
  const waiver = { ...marker };
  delete waiver.asked_at;
  waiver.waived_at = now(options);
  if (!write(pushWaiverPath(root, slug, options), waiver))
    return false;
  rmSync3(pending, { force: true });
  return true;
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

// packages/toolu-core/src/gates/push-waiver.ts
var ALLOW3 = { kind: "allow" };
function pushFailed(status) {
  return status !== "" && status !== "null" && status !== "0";
}
function decide2(event, ctx) {
  if (!isShellTool(event))
    return ALLOW3;
  const analysis = commandAnalysis(event, ctx);
  if (!isGitPush(analysis))
    return ALLOW3;
  if (pushFailed(toolExitStatus(ctx.raw)) || toolInterrupted(ctx.raw))
    return ALLOW3;
  const cwd = ctx.cwd ?? process.cwd();
  const root = pushTargetRoot(analysis, { env: ctx.env, cwd });
  const branch = currentBranch(root, ctx.env);
  if (branch === "" || branch === "HEAD")
    return ALLOW3;
  const base = envValue(ctx.env, "PUSH_REVIEW_BASE") ?? baseBranch(root, ctx.env);
  const sha = diffSha(root, base, { env: ctx.env });
  if (sha === undefined)
    return ALLOW3;
  pushWaiverPromote(root, branchSlug(branch), sha, { env: ctx.env, host: ctx.host });
  return ALLOW3;
}
var pushWaiverModule = {
  kind: "native",
  name: "push-waiver.sh",
  run: (event, ctx) => Promise.resolve(decide2(event, ctx))
};
// packages/toolu-core/src/gates/bash-pattern.ts
var CLASSES = {
  alnum: /[\p{L}\p{N}]/u,
  alpha: /\p{L}/u,
  blank: /[ \t]/,
  cntrl: /\p{Cc}/u,
  digit: /[0-9]/,
  graph: /[^\s\p{Cc}]/u,
  lower: /\p{Ll}/u,
  print: /[^\p{Cc}]/u,
  punct: /[!-/:-@[-`{-~]/,
  space: /\s/,
  upper: /\p{Lu}/u,
  word: /[\p{L}\p{N}_]/u,
  xdigit: /[0-9A-Fa-f]/
};
function bracketMember(chars, at) {
  const open = chars[at];
  const kind = chars[at + 1];
  if (open === "[" && (kind === ":" || kind === "=" || kind === ".")) {
    const close = chars.indexOf(kind, at + 2);
    if (close !== -1 && chars[close + 1] === "]") {
      const name = chars.slice(at + 2, close).join("");
      const next = close + 2;
      if (kind === ":") {
        const cls = CLASSES[name];
        if (name === "ascii")
          return { next, test: (c) => (c.codePointAt(0) ?? 128) < 128 };
        return cls === undefined ? { next, test: () => false } : { next, test: (c) => cls.test(c) };
      }
      return { next, test: (c) => c === name, char: name };
    }
  }
  if (open === "\\" && at + 1 < chars.length) {
    const char = chars[at + 1] ?? "";
    return { next: at + 2, test: (c) => c === char, char };
  }
  if (open === undefined)
    return;
  return { next: at + 1, test: (c) => c === open, char: open };
}
function parseBracket(chars, start) {
  let at = start + 1;
  const negated = chars[at] === "!" || chars[at] === "^";
  if (negated)
    at += 1;
  const tests = [];
  let first = true;
  while (at < chars.length) {
    if (chars[at] === "]" && !first) {
      const bracket = { negated, test: (c) => tests.some((t) => t(c)) };
      return { next: at + 1, bracket };
    }
    first = false;
    const member = bracketMember(chars, at);
    if (member === undefined)
      return;
    const low = member.char;
    if (low !== undefined && chars[member.next] === "-" && chars[member.next + 1] !== "]") {
      const high = bracketMember(chars, member.next + 1);
      if (high?.char !== undefined) {
        const lo = low.codePointAt(0) ?? 0;
        const hi = high.char.codePointAt(0) ?? 0;
        tests.push((c) => {
          const code = c.codePointAt(0) ?? -1;
          return code >= lo && code <= hi;
        });
        at = high.next;
        continue;
      }
    }
    tests.push(member.test);
    at = member.next;
  }
  return;
}
function extglobEnd(chars, start) {
  let depth = 0;
  const bars = [];
  let at = start;
  while (at < chars.length) {
    const c = chars[at];
    if (c === "\\") {
      at += 2;
      continue;
    }
    if (c === "[") {
      const bracket = parseBracket(chars, at);
      if (bracket !== undefined) {
        at = bracket.next;
        continue;
      }
    }
    if (c === "(")
      depth += 1;
    if (c === ")") {
      if (depth === 0)
        return { end: at, bars };
      depth -= 1;
    }
    if (c === "|" && depth === 0)
      bars.push(at);
    at += 1;
  }
  return;
}
var EXT_OPS = new Set(["?", "*", "+", "@", "!"]);
function isExtOp(c) {
  return c !== undefined && EXT_OPS.has(c);
}
function parse5(chars, from, to) {
  const nodes = [];
  let at = from;
  while (at < to) {
    const c = chars[at] ?? "";
    if (isExtOp(c) && chars[at + 1] === "(") {
      const close = extglobEnd(chars, at + 2);
      if (close !== undefined && close.end < to) {
        const bounds = [at + 1, ...close.bars, close.end];
        const alternatives = bounds.slice(1).map((end, i) => parse5(chars, (bounds[i] ?? 0) + 1, end));
        nodes.push({ kind: "ext", op: c, alternatives });
        at = close.end + 1;
        continue;
      }
    }
    if (c === "*") {
      nodes.push({ kind: "star" });
    } else if (c === "?") {
      nodes.push({ kind: "any" });
    } else if (c === "[") {
      const bracket = parseBracket(chars, at);
      if (bracket !== undefined && bracket.next <= to) {
        nodes.push({ kind: "bracket", bracket: bracket.bracket });
        at = bracket.next;
        continue;
      }
      nodes.push({ kind: "literal", char: c });
    } else if (c === "\\" && at + 1 < to) {
      nodes.push({ kind: "literal", char: chars[at + 1] ?? "" });
      at += 2;
      continue;
    } else {
      nodes.push({ kind: "literal", char: c });
    }
    at += 1;
  }
  return nodes;
}
function anyAlt(alternatives, span, start, end) {
  return alternatives.some((alt) => matchSeq(alt, span.text, start, end));
}
function matchExt(node, span, pos, rest) {
  const { op, alternatives } = node;
  if (op === "!") {
    for (let end = pos;end <= span.to; end += 1) {
      if (!anyAlt(alternatives, span, pos, end) && rest(end))
        return true;
    }
    return false;
  }
  if ((op === "?" || op === "*") && rest(pos))
    return true;
  for (let end = pos + (op === "@" || op === "?" ? 0 : 1);end <= span.to; end += 1) {
    if (!anyAlt(alternatives, span, pos, end))
      continue;
    if (rest(end))
      return true;
    const again = (op === "*" || op === "+") && end > pos;
    if (again && matchExt({ ...node, op: "*" }, span, end, rest))
      return true;
  }
  return false;
}
function matchSeq(nodes, text, from, to) {
  const span = { text, to };
  const memo = new Map;
  const step = (i, pos) => {
    const key = i * (to + 1) + pos;
    const cached = memo.get(key);
    if (cached !== undefined)
      return cached;
    const result = stepAt(i, pos);
    memo.set(key, result);
    return result;
  };
  const stepAt = (i, pos) => {
    const node = nodes[i];
    if (node === undefined)
      return pos === to;
    const char = text[pos];
    switch (node.kind) {
      case "literal":
        return char === node.char && step(i + 1, pos + 1);
      case "any":
        return pos < to && step(i + 1, pos + 1);
      case "bracket":
        return char !== undefined && pos < to && node.bracket.test(char) !== node.bracket.negated && step(i + 1, pos + 1);
      case "star":
        for (let end = pos;end <= to; end += 1)
          if (step(i + 1, end))
            return true;
        return false;
      case "ext":
        return matchExt(node, span, pos, (end) => step(i + 1, end));
      default: {
        const never = node;
        return never;
      }
    }
  };
  return step(0, from);
}
function compileBashPattern(pattern) {
  const chars = Array.from(pattern);
  const nodes = parse5(chars, 0, chars.length);
  return (text) => {
    const units = Array.from(text);
    return matchSeq(nodes, units, 0, units.length);
  };
}
// packages/toolu-core/src/gates/code-edit-rules.ts
var EDIT_TOOLS2 = new Set(["Edit", "Write", "MultiEdit"]);
// packages/toolu-core/src/gates/protected-files.ts
var EDIT_TOOLS3 = new Set(["Edit", "Write", "MultiEdit"]);
var DETAILS = [
  [
    compileBashPattern("@(*.env.example|*.env.template|*.env.sample)"),
    "This is an example/template env file. It is committed on purpose, so it should carry placeholders and never live values \u2014 it is guarded because a real credential pasted here is a credential published to the repo."
  ],
  [
    compileBashPattern("@(.env|.env.*|*secrets*)"),
    "This is a secrets file. Approving lets an agent read or rewrite live credentials, and anything it writes here can leak into logs, commits, or a diff you push."
  ],
  [
    compileBashPattern("@(.git/*|*/.git/*)"),
    "This is git's internal state. Approving lets an agent rewrite refs, hooks, or config \u2014 including hooks that run on your machine at every commit."
  ],
  [
    compileBashPattern("@(*hooks/*|*skills/*)"),
    "This is toolu's own enforcement code \u2014 the hooks that run every other gate. Approving lets an agent edit the thing that is supposed to be watching it, which is how a guardrail gets quietly switched off."
  ]
];
// packages/toolu-core/src/ledger/ledger-parse.ts
var SPACE = "[ \\t\\n\\v\\f\\r]";
var STEPS_HEADING = new RegExp(`^## Steps \\(machine-readable\\)${SPACE}*$`);
var JSON_FENCE = new RegExp(`^\`\`\`json${SPACE}*$`);
var CLOSE_FENCE = new RegExp(`^\`\`\`${SPACE}*$`);
var AC_HEADING = new RegExp(`^## Acceptance criteria${SPACE}*$`);
// packages/toolu-core/src/ledger/review-state.ts
var ACCEPTED_REVIEWERS = [
  "code-review",
  "toolu-review:review",
  "code-review:xhigh",
  "review",
  "security-review"
];

// packages/toolu-core/src/gates/push-review-state.ts
var REVIEWER_LIST = JSON.stringify(ACCEPTED_REVIEWERS);
// plugins/toolu/hooks/src/post-tools/builtins.ts
function builtins() {
  return [gateStatusModule, pushWaiverModule];
}

// plugins/toolu/hooks/src/pre-tools/hook-main.ts
import { dirname as dirname4 } from "path";
async function hookMain(entryDir, run, event = "PreToolUse") {
  try {
    const result = await run(await Bun.stdin.text(), dirname4(entryDir));
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
    process.exitCode = result.exitCode;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const prefix = event === "PreToolUse" ? "blocked: " : "";
    process.stderr.write(`${prefix}toolu ${event} dispatcher failed: ${message}
`);
    process.exitCode = 2;
  }
}

// plugins/toolu/hooks/src/post-tools.ts
await hookMain(import.meta.dir, (stdin, hooks) => dispatchPostTool(stdin, { builtins: builtins(), libDir: join9(hooks, "lib") }), "PostToolUse");
