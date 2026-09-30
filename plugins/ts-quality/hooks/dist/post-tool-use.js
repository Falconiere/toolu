// @bun
// plugins/ts-quality/hooks/src/post-tool-use.ts
import { readFileSync as readFileSync7 } from "fs";

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
  const files = {
    user: join2(configRoot(scoped), "toolu.config.json"),
    project: projectConfigPath(scoped)
  };
  return { files, host };
}

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
// packages/toolu-core/src/config/quality-config.ts
import { readdirSync, readFileSync as readFileSync2 } from "fs";
import { join as join3 } from "path";
var QUALITY_DEFAULTS = {
  ts: { maxFileLines: 300, maxFnLines: 60 },
  rust: { maxFileLines: 500, maxFnLines: 50, maxImplLines: 200 },
  python: { maxFileLines: 400, maxFnLines: 50 }
};
var JQ_NUMBER = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;
function positiveFloor(value) {
  let number = value;
  if (typeof value === "string") {
    const trimmed = value.trim();
    number = JQ_NUMBER.test(trimmed) ? Number(trimmed) : undefined;
  }
  return typeof number === "number" && number > 0 ? Math.floor(number) : undefined;
}
function langMember(config, lang, key) {
  const entry = section(config, "lang")?.[lang];
  return isJsonObject(entry) ? entry[key] : undefined;
}
function nativeMaxLines(linterConfig) {
  const rules = isJsonObject(linterConfig) ? linterConfig.rules : undefined;
  const rule = isJsonObject(rules) ? rules["max-lines"] : undefined;
  if (Array.isArray(rule)) {
    const items = rule;
    const severity = items[0];
    const option = items[1];
    if (severity === "off" || severity === 0) {
      return;
    }
    return positiveFloor(isJsonObject(option) ? option.max : option);
  }
  return typeof rule === "number" || typeof rule === "string" ? positiveFloor(rule) : undefined;
}
function activeLinterConfig(root) {
  if (isFile(join3(root, "biome.json")) || isFile(join3(root, "biome.jsonc"))) {
    return;
  }
  if (isFile(join3(root, ".oxlintrc.json"))) {
    return join3(root, ".oxlintrc.json");
  }
  let names;
  try {
    names = readdirSync(root);
  } catch {
    return;
  }
  const eslint = names.some((name) => name.startsWith(".eslintrc") || name.startsWith("eslint.config."));
  return eslint ? join3(root, ".eslintrc.json") : undefined;
}
function nativeTsMaxLines(options) {
  const root = options.root ?? gitToplevel(options.env ?? process.env, options.cwd);
  const file = root === undefined ? undefined : activeLinterConfig(root);
  if (file === undefined || !isFile(file)) {
    return;
  }
  try {
    return nativeMaxLines(JSON.parse(readFileSync2(file, "utf8")));
  } catch {
    return;
  }
}
function qualityThreshold(config, lang, key, options = {}) {
  const override = positiveFloor(langMember(config, lang, key));
  if (override !== undefined) {
    return override;
  }
  if (lang === "ts" && key === "maxFileLines") {
    const native = nativeTsMaxLines(options);
    if (native !== undefined) {
      return native;
    }
  }
  const defaults = QUALITY_DEFAULTS[lang];
  return defaults[key] ?? 0;
}
function tsMaxFileLinesResolved(config, options = {}) {
  const override = positiveFloor(langMember(config, "ts", "maxFileLines"));
  if (override !== undefined) {
    return { value: override, source: "override" };
  }
  const native = nativeTsMaxLines(options);
  return native === undefined ? { value: QUALITY_DEFAULTS.ts.maxFileLines, source: "default" } : { value: native, source: "native" };
}
function qualityFlag(config, lang, key, fallback) {
  const value = langMember(config, lang, key);
  return typeof value === "boolean" ? value : fallback;
}
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
  opencode: new Set(["tool/pre", "shell/pre", "permission/evaluate"])
};
// packages/toolu-core/src/config/settings.ts
var CodeEditRulesSchema = object({
  rules: array(object({
    match: string2(),
    docs: array(string2()),
    when_path_matches: array(string2()).optional(),
    extra_docs: array(string2()).optional()
  }).strict())
});
// packages/toolu-core/src/detect/detect-branch.ts
function branchSlug(branch) {
  const slug = branch.replaceAll("/", "_").replace(/[^A-Za-z0-9_-]/g, "");
  return slug === "" ? "_default" : slug;
}
// packages/toolu-core/src/detect/detect-read.ts
import { closeSync, openSync, readSync, statSync as statSync2 } from "fs";
var CHUNK = 64 * 1024;
var NEWLINE = 10;
function isRegularFile(path) {
  try {
    return statSync2(path).isFile();
  } catch {
    return false;
  }
}
function eachLine(path, visit) {
  let fd;
  try {
    fd = openSync(path, "r");
  } catch {
    return "unreadable";
  }
  try {
    return walkLines(fd, visit);
  } catch (error) {
    const code = errnoCode(error);
    if (code === undefined)
      throw error;
    return code === "EISDIR" ? "done" : "unreadable";
  } finally {
    closeSync(fd);
  }
}
function errnoCode(error) {
  return error instanceof Error && "code" in error && typeof error.code === "string" ? error.code : undefined;
}
function walkLines(fd, visit) {
  const buf = Buffer.allocUnsafe(CHUNK);
  let carry = "";
  for (let n = readSync(fd, buf, 0, CHUNK, null);n > 0; n = readSync(fd, buf, 0, CHUNK, null)) {
    let start = 0;
    for (let nl = buf.indexOf(NEWLINE, 0);nl !== -1 && nl < n; nl = buf.indexOf(NEWLINE, start)) {
      const line = carry + buf.toString("latin1", start, nl);
      carry = "";
      start = nl + 1;
      if (visit(line) === true)
        return "stopped";
    }
    carry += buf.toString("latin1", start, n);
  }
  if (carry !== "" && visit(carry) === true)
    return "stopped";
  return "done";
}

// packages/toolu-core/src/detect/detect-lines.ts
function trimBlanks(line) {
  return line.replace(/^[ \t]+|[ \t]+$/g, "");
}
function stripBlocks(line, inBlock) {
  let rest = line;
  if (inBlock) {
    const close = rest.indexOf("*/");
    if (close === -1)
      return;
    rest = rest.slice(close + 2);
  }
  for (;; ) {
    const open = rest.indexOf("/*");
    if (open === -1)
      return { code: rest, inBlock: false };
    const after = rest.slice(open + 2);
    const close = after.indexOf("*/");
    if (close === -1)
      return { code: rest.slice(0, open), inBlock: true };
    rest = rest.slice(0, open) + after.slice(close + 2);
  }
}
function countCodeLines(path) {
  const count = { inBlock: false, records: 0, code: 0 };
  const walk = eachLine(path, (line) => {
    count.records += 1;
    const stripped = stripBlocks(line, count.inBlock);
    if (stripped === undefined)
      return;
    count.inBlock = stripped.inBlock;
    const comment = stripped.code.indexOf("//");
    const kept = comment === -1 ? stripped.code : stripped.code.slice(0, comment);
    if (trimBlanks(kept) !== "")
      count.code += 1;
  });
  if (walk === "unreadable")
    return;
  return count.inBlock ? count.records : count.code;
}
function occurrences(line, token) {
  let count = 0;
  for (let at = line.indexOf(token);at !== -1; at = line.indexOf(token, at + token.length)) {
    count += 1;
  }
  return count;
}
function hasUnterminatedBlock(path) {
  if (!isRegularFile(path))
    return false;
  let balance = 0;
  eachLine(path, (line) => {
    balance += occurrences(line, "/*") - occurrences(line, "*/");
  });
  return balance > 0;
}
// packages/toolu-core/src/detect/detect-project.ts
import { spawnSync as spawnSync2 } from "child_process";
import { readdirSync as readdirSync2 } from "fs";
import { basename, join as join4 } from "path";
function projectToplevel(options = {}) {
  return gitToplevel(options.env ?? process.env, options.cwd);
}
function hasAny(root, names) {
  return names.some((name) => isRegularFile(join4(root, name)));
}
var LOCK_FILES = [
  ["bun.lock", "bun"],
  ["bun.lockb", "bun"],
  ["pnpm-lock.yaml", "pnpm"],
  ["yarn.lock", "yarn"],
  ["package-lock.json", "npm"]
];
function nodePackageManager(options = {}) {
  const root = projectToplevel(options);
  if (root === undefined)
    return;
  return LOCK_FILES.find(([file]) => isRegularFile(join4(root, file)))?.[1];
}
function detectTs(options = {}) {
  const env = options.env ?? process.env;
  const root = gitToplevel(env, options.cwd);
  if (root === undefined)
    return false;
  const res = spawnSync2("git", ["-C", root, "ls-files", "**/tsconfig*.json", "tsconfig*.json"], {
    env: childEnv(env),
    encoding: "utf8"
  });
  return res.error === undefined && /[^\n]/.test(res.stdout);
}
function entries(root) {
  try {
    return readdirSync2(root);
  } catch {
    return [];
  }
}
function tsLinter(options = {}) {
  const root = projectToplevel(options);
  if (root === undefined)
    return;
  if (hasAny(root, ["biome.json", "biome.jsonc"]))
    return "biome";
  if (hasAny(root, [".oxlintrc.json"]))
    return "oxc";
  const eslint = entries(root).some((name) => name.startsWith(".eslintrc") || name.startsWith("eslint.config."));
  return eslint ? "eslint" : undefined;
}
// packages/toolu-core/src/detect/detect-tools.ts
import { accessSync, constants, statSync as statSync3 } from "fs";
import { join as join5 } from "path";
var cache = new Map;
function isCommandFile(path) {
  try {
    return !statSync3(path).isDirectory();
  } catch {
    return false;
  }
}
function isExecutable(path) {
  try {
    accessSync(path, constants.X_OK);
    return !statSync3(path).isDirectory();
  } catch {
    return false;
  }
}
function scan(name, dirs) {
  return dirs.some((dir) => isCommandFile(join5(dir === "" ? "." : dir, name)));
}
function toolAvailable(name, env = process.env) {
  if (name === "")
    return false;
  if (name.includes("/"))
    return isExecutable(name);
  const path = envValue(env, "PATH") ?? "";
  const dirs = path.split(":");
  if (dirs.some((dir) => !dir.startsWith("/")))
    return scan(name, dirs);
  const key = `${path}\x00${name}`;
  const hit = cache.get(key);
  if (hit !== undefined)
    return hit;
  const found = scan(name, dirs);
  cache.set(key, found);
  return found;
}
// packages/toolu-core/src/quality/quality-ast-grep.ts
import { constants as constants2 } from "os";
function interpolated(value) {
  return typeof value === "string" ? value : JSON.stringify(value ?? null);
}
function matchHits(match) {
  if (!isJsonObject(match))
    return;
  const lines = match.lines === undefined || match.lines === null || match.lines === false ? "" : match.lines;
  const range = match.range;
  const start = isJsonObject(range) && isJsonObject(range.start) ? range.start.line : undefined;
  if (typeof lines !== "string" || typeof start !== "number")
    return;
  const texts = lines === "" ? [] : lines.split(`
`);
  const ruleId = interpolated(match.ruleId);
  const file = interpolated(match.file);
  return texts.map((text, key) => ({
    ruleId,
    line: start + 1 + key,
    excerpt: `${file}:${String(start + 1 + key)}:${text}`
  }));
}
function parseHits(stdout) {
  let doc;
  try {
    doc = JSON.parse(stdout);
  } catch {
    return;
  }
  if (!Array.isArray(doc))
    return;
  const hits = [];
  for (const match of doc) {
    const flat = matchHits(match);
    if (flat === undefined)
      return;
    hits.push(...flat);
  }
  return hits;
}
function exitStatus(res) {
  if (res.exitCode !== null)
    return res.exitCode;
  const signals = constants2.signals;
  return 128 + (signals[res.signalCode ?? ""] ?? 0);
}
function astGrepScan(file, rules, ctx) {
  const cwd = ctx.cwd ?? process.cwd();
  const bin = Bun.which("ast-grep", { PATH: envValue(ctx.env, "PATH") ?? "", cwd });
  if (bin === null)
    return { kind: "missing" };
  const res = Bun.spawnSync([bin, "scan", "--inline-rules", rules, "--json", file.path], {
    cwd,
    env: childEnv(ctx.env),
    stdout: "pipe",
    stderr: "pipe"
  });
  const stdout = res.stdout.toString();
  const stderr = res.stderr.toString();
  const exitCode = exitStatus(res);
  if (exitCode !== 0 || stderr !== "") {
    const stderrFirst = (stderr.split(`
`)[0] ?? "").slice(0, 200);
    return { kind: "failed", stage: "ast-grep", exitCode, stderrFirst };
  }
  if (stdout.trim() === "")
    return { kind: "ok", hits: [], empty: true };
  const hits = parseHits(stdout);
  if (hits === undefined)
    return { kind: "failed", stage: "parse", exitCode, stderrFirst: "" };
  return { kind: "ok", hits, empty: false };
}
// packages/toolu-core/src/quality/quality-edit.ts
import { spawnSync as spawnSync3 } from "child_process";
import { statSync as statSync5 } from "fs";
import { dirname, resolve } from "path";

// packages/toolu-core/src/state/state-io.ts
import { randomUUID } from "crypto";
import { linkSync, readFileSync as readFileSync3, renameSync, rmSync, statSync as statSync4, writeFileSync } from "fs";
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
    return readFileSync3(lock, "utf8");
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
  const stat = statSync4(lock, { throwIfNoEntry: false });
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

// packages/toolu-core/src/quality/quality-edit.ts
var EDIT_TOOLS = new Set(["Write", "Edit", "MultiEdit"]);
function inputField(ctx, keys) {
  const input = ctx.raw.tool_input;
  if (!isJsonObject(input))
    return "";
  for (const key of keys) {
    const value = input[key];
    if (value === undefined || value === null || value === false)
      continue;
    const text = typeof value === "string" ? value : toJqJson(value, true);
    return text.replace(/\n+$/, "");
  }
  return "";
}
function editField(ctx, split, name) {
  const exported = ctx.edit === undefined ? envValue(ctx.env, `TOOLU_EDIT_${name}`) : split;
  if (exported !== undefined && exported !== "")
    return exported;
  return inputField(ctx, [`toolu_edit_${name.toLowerCase()}`]);
}
function editedFile(event, ctx) {
  const fromInput = EDIT_TOOLS.has(event.toolName) ? inputField(ctx, ["path", "file_path", "target_file"]) : "";
  const path = envValue(ctx.env, "CLAUDE_FILE_PATHS") ?? fromInput;
  if (path === "")
    return;
  const operation = editField(ctx, ctx.edit?.operation, "OPERATION");
  const movedTo = editField(ctx, ctx.edit?.movedTo, "MOVED_TO");
  return {
    path,
    absolute: resolve(ctx.cwd ?? process.cwd(), path),
    removed: operation === "delete" || movedTo !== ""
  };
}
function isRegularFile2(file) {
  try {
    return statSync5(file.absolute).isFile();
  } catch {
    return false;
  }
}
function withoutSlash(dir) {
  return dir.replace(/\/$/, "");
}
function inLinkedWorktree(file, ctx) {
  const res = spawnSync3("git", [
    "-C",
    dirname(file.path),
    "rev-parse",
    "--path-format=absolute",
    "--git-dir",
    "--git-common-dir"
  ], { cwd: ctx.cwd, env: childEnv(ctx.env), encoding: "utf8" });
  const [gitDir = "", commonDir = ""] = res.stdout.split(`
`);
  return gitDir !== "" && commonDir !== "" && withoutSlash(gitDir) !== withoutSlash(commonDir);
}
// packages/toolu-core/src/quality/quality-gate.ts
import { mkdirSync as mkdirSync2 } from "fs";
import { dirname as dirname3, join as join7 } from "path";

// packages/toolu-core/src/state/gate-file.ts
import { appendFileSync as appendFileSync2, existsSync, readFileSync as readFileSync4, writeFileSync as writeFileSync2 } from "fs";
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
import { appendFileSync, mkdirSync } from "fs";
import { join as join6 } from "path";

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
  const file = join6(dir, `${branchSlug(branch)}.jsonl`);
  try {
    mkdirSync(dir, { recursive: true });
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
    value = JSON.parse(readFileSync4(gateFile, "utf8"));
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
    writeFileSync2(gateFile, `${toJqJson(doc, true)}
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

// packages/toolu-core/src/quality/quality-gate.ts
function qualityGateFile(ctx) {
  const dir = projectStateRoot({ env: ctx.env, host: ctx.host, root: ctx.projectRoot });
  if (dir === undefined)
    throw new Error("quality: no project state root");
  return join7(dir, "quality-gate-status.json");
}
function clearQualityEntry(ctx, file, source) {
  clearGateFile(qualityGateFile(ctx), file.path, source, { env: ctx.env, host: ctx.host });
}
function settleQuality(ctx, file, gate, findings) {
  if (findings.errors.length > 0) {
    const messages = findings.errors.map((error) => `${error}
`).join("");
    const gateFile = qualityGateFile(ctx);
    mkdirSync2(dirname3(gateFile), { recursive: true });
    const state = { env: ctx.env, host: ctx.host };
    recordGateFailure(gateFile, file.path, gate.source, gate.reason, messages, state);
    return {
      kind: "advisory",
      message: `QUALITY VIOLATION \u2014 fix before proceeding:
${messages}`
    };
  }
  clearQualityEntry(ctx, file, gate.source);
  const advisory = findings.advisories.filter((text) => text !== "").join(`
`);
  return advisory === "" ? { kind: "allow" } : { kind: "advisory", message: advisory };
}
// packages/toolu-core/src/quality/quality-run.ts
var ALLOW = { kind: "allow" };
function fileQuality(event, ctx, spec) {
  const file = editedFile(event, ctx);
  if (file === undefined)
    return ALLOW;
  if (file.removed) {
    if (spec.matches.test(file.path))
      clearQualityEntry(ctx, file, spec.source);
    return ALLOW;
  }
  if (!isRegularFile2(file) || !spec.matches.test(file.path))
    return ALLOW;
  if (spec.skipLinkedWorktrees && inLinkedWorktree(file, ctx))
    return ALLOW;
  return settleQuality(ctx, file, spec, spec.check(file));
}
// packages/toolu-core/src/host/host-snapshot.ts
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
// packages/toolu-core/src/registry/registry-paths.ts
var EVENT_DIRS = {
  "tool/pre": "pre-tools.d",
  "tool/post": "post-tools.d"
};
var REGISTRY_DIRS = Object.values(EVENT_DIRS);
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

// packages/toolu-core/src/registry/registry-types.ts
var REGISTRY_EVENTS = ["tool/pre", "tool/post"];
function defineRegistryModule(module) {
  return module;
}

// packages/toolu-core/src/registry/registry-run.ts
var ModuleSchema = looseObject({
  spec: string2(),
  name: string2(),
  event: _enum(REGISTRY_EVENTS),
  run: custom((value) => typeof value === "function")
});
// plugins/ts-quality/hooks/src/rules/advisories.ts
import { spawnSync as spawnSync5 } from "child_process";
import { statSync as statSync6 } from "fs";
import { basename as basename2, join as join8 } from "path";

// plugins/ts-quality/hooks/src/rules/ts-file.ts
var SP = "[ \\t\\n\\v\\f\\r]";
function ere(source) {
  return new RegExp(source.replaceAll("[[:space:]]", SP), "s");
}
function splitLines(text) {
  if (text === "")
    return [];
  const lines = text.split(`
`);
  if (text.endsWith(`
`))
    lines.pop();
  return lines;
}
function numbered(lines, pattern) {
  return lines.flatMap((line, i) => pattern.test(line) ? [`${String(i + 1)}:${line}`] : []);
}
function countMatches(lines, pattern) {
  return lines.filter((line) => pattern.test(line)).length;
}
function head(items, n) {
  return items.slice(0, n).join(`
`);
}
function withoutCommentLines(rows) {
  const comment = ere("^[0-9]+:[[:space:]]*//");
  return rows.filter((row) => !comment.test(row));
}
function withExcerpt(header, excerpt) {
  return excerpt === "" ? undefined : `${header}
${excerpt}`;
}
function pathHas(f, part) {
  return f.file.path.includes(part);
}
function isTestPath(path) {
  return /\.(test|spec)\.(ts|tsx)$/s.test(path);
}
function spawnEnv(f) {
  const env = {};
  for (const [key, value] of Object.entries(f.ctx.env))
    if (value !== undefined)
      env[key] = value;
  return env;
}

// plugins/ts-quality/hooks/src/rules/advisories.ts
function typecheckCommand(pm) {
  if (pm === "bun")
    return "bun run typecheck";
  if (pm === "pnpm")
    return "pnpm -w typecheck";
  if (pm === "yarn")
    return "yarn typecheck";
  if (pm === "npm")
    return "npm run typecheck";
  return `${pm} run typecheck`;
}
var RUNNERS = {
  bun: ["bunx", ["bunx", "jscpd"]],
  pnpm: ["pnpm", ["pnpm", "dlx", "jscpd"]],
  yarn: ["yarn", ["yarn", "dlx", "jscpd"]],
  npm: ["npx", ["npx", "jscpd"]]
};
function kindOf(path) {
  try {
    const stat = statSync6(path);
    return stat.isFile() ? "file" : stat.isDirectory() ? "dir" : "none";
  } catch {
    return "none";
  }
}
function bre(text) {
  const chars = [...text];
  const escaped = chars.map((char, at) => {
    if ("+?(){}|".includes(char))
      return `\\${char}`;
    if (char === "^" && at > 0)
      return String.raw`\^`;
    if (char === "$" && at < chars.length - 1)
      return String.raw`\$`;
    return char;
  });
  return new RegExp(escaped.join(""), "s");
}
function jscpdOutput(f, runner, pkg, config) {
  const res = spawnSync5("timeout", ["10", ...runner, pkg, "--config", config], {
    cwd: f.ctx.cwd,
    env: spawnEnv(f),
    encoding: "utf8"
  });
  const out = res.stdout ?? "";
  return `${out}${out === "" || out.endsWith(`
`) ? "" : `
`}${res.stderr ?? ""}`;
}
function duplication(f) {
  const path = f.file.path;
  if (/\.(test|spec)\./s.test(path))
    return "";
  const root = f.ctx.projectRoot;
  const relative = path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path;
  if (!relative.startsWith("apps/") && !relative.startsWith("packages/"))
    return "";
  const pkg = `${root}/${relative.split("/").slice(0, 2).join("/")}`;
  const runner = RUNNERS[f.pm];
  const config = join8(root, ".jscpd.json");
  if (runner === undefined || !toolAvailable(runner[0], f.ctx.env))
    return "";
  if (kindOf(pkg) !== "dir" || kindOf(config) !== "file")
    return "";
  const lines = jscpdOutput(f, runner[1], pkg, config).split(`
`);
  const cloned = lines.some((line) => /found.*clone|duplicat/is.test(line));
  const named = lines.some((line) => bre(basename2(path)).test(line));
  if (!cloned || !named)
    return "";
  return `Code duplication detected involving ${path} \u2014 deduplicate or run '${typecheckCommand(f.pm)}' before commit`;
}
var EXPORTS = [
  /^export (async )?function /s,
  /^export (abstract )?class /s,
  /^export default /s,
  /^export (const|interface|type|enum) [A-Z]/s,
  /^export const [a-z_][A-Za-z0-9_]* = (async )?(\(|function)/s
];
function undocumented(lines) {
  const rows = [];
  const blank = ere("^[[:space:]]*$");
  const lineComment = ere("^[[:space:]]*//");
  let prev = "";
  lines.forEach((line, i) => {
    if (blank.test(line) || lineComment.test(line))
      return;
    const documented = ere(String.raw`\*/[[:space:]]*$`).test(prev) || ere(String.raw`^[[:space:]]*/\*\*`).test(prev);
    if (EXPORTS.some((pattern) => pattern.test(line)) && !documented)
      rows.push(`${String(i + 1)}: ${line}`);
    prev = line;
  });
  return rows;
}
function verbose(lines) {
  const rows = [];
  let block;
  lines.forEach((line, i) => {
    if (block === undefined && line.includes("/**"))
      block = { start: i + 1, count: 0 };
    if (block === undefined)
      return;
    block.count += 1;
    if (!line.includes("*/"))
      return;
    if (block.count > 12)
      rows.push(`${String(block.start)}: JSDoc block is ${String(block.count)} lines \u2014 trim to the essentials`);
    block = undefined;
  });
  return rows;
}
function docs(f) {
  const path = f.file.path;
  const name = basename2(path);
  if (isTestPath(path) || /\.d\.ts$/s.test(path) || name === "index.ts" || name === "index.tsx")
    return "";
  const missing = head(undocumented(f.lines), 3);
  const long = head(verbose(f.lines), 2);
  const parts = [];
  if (missing !== "")
    parts.push(`Exported API missing a JSDoc in ${path} \u2014 add a concise /** */ doc:
${missing}`);
  if (long !== "")
    parts.push(`Verbose JSDoc in ${path} \u2014 docs must be present but concise:
${long}`);
  return parts.join(`
`);
}
function unhandledAwait(f) {
  const comment = ere(String.raw`^[[:space:]]*(//|/\*|\*)`);
  const code = f.lines.filter((line) => !comment.test(line));
  const awaits = code.some((line) => ere(String.raw`\bawait[[:space:]]`).test(line));
  const handled = code.some((line) => /\btry\b|\.catch\(/s.test(line));
  if (!awaits || handled)
    return "";
  return `Async code in ${f.file.path} uses await with no try/catch or .catch in the file \u2014 ensure rejections are handled here or by every caller.`;
}

// plugins/ts-quality/hooks/src/rules/ast-rule-yaml.ts
var ERROR_RULES = `id: empty-catch
language: ts
severity: warning
message: empty catch
rule:
  pattern: 'try { $$$ } catch ($_) { }'
---
id: empty-catch-noarg
language: ts
severity: warning
message: empty catch (no binding)
rule:
  pattern: 'try { $$$ } catch { }'
---
id: empty-catch-handler
language: ts
severity: warning
message: empty promise catch handler
rule:
  pattern: '$_.catch(() => { })'
---
id: null-catch-handler
language: ts
severity: warning
message: promise catch handler returning null
rule:
  pattern: '$_.catch(() => null)'
---
id: undef-catch-handler
language: ts
severity: warning
message: promise catch handler returning undefined
rule:
  pattern: '$_.catch(() => undefined)'
---
id: swallow-null-arg
language: ts
severity: warning
message: catch returns null
rule:
  pattern: 'try { $$$ } catch ($_) { return null }'
---
id: swallow-undef-arg
language: ts
severity: warning
message: catch returns undefined
rule:
  pattern: 'try { $$$ } catch ($_) { return undefined }'
---
id: swallow-null
language: ts
severity: warning
message: catch returns null (no binding)
rule:
  pattern: 'try { $$$ } catch { return null }'
---
id: swallow-undef
language: ts
severity: warning
message: catch returns undefined (no binding)
rule:
  pattern: 'try { $$$ } catch { return undefined }'
---
# A bare \`return\` in an ast-grep pattern acts as a WILDCARD over the optional
# argument: \`catch { return }\` also matched \`return []\`, \`return 42\` and
# \`return null\`, i.e. every non-nullish fallback got reported as "returns a
# nullish value". Match the catch clause itself, then require that its single
# return statement carries no argument at all.
id: swallow-bare
language: ts
severity: warning
message: catch returns nothing
rule:
  all:
    - any:
        - pattern:
            context: 'try {} catch { return }'
            selector: catch_clause
        - pattern:
            context: 'try {} catch ($_) { return }'
            selector: catch_clause
    - not:
        has:
          stopBy: end
          kind: return_statement
          has:
            stopBy: neighbor
            pattern: $X
---
id: throw-empty-error
language: ts
severity: warning
message: throw new Error() with no message
rule:
  pattern: 'throw new Error()'
---
id: throw-string
language: ts
severity: warning
message: throw of a string literal
rule:
  pattern: 'throw "$S"'
---
id: throw-template
language: ts
severity: warning
message: throw of a template literal
rule:
  pattern: 'throw \`$S\`'`;
var MOCK_RULES = `id: jest-mock
language: ts
severity: warning
message: jest.mock() call
rule:
  pattern: jest.mock($$$)
---
id: vi-mock
language: ts
severity: warning
message: vi.mock() call
rule:
  pattern: vi.mock($$$)
---
id: jest-fn
language: ts
severity: warning
message: jest.fn() call
rule:
  pattern: jest.fn($$$)
---
id: vi-fn
language: ts
severity: warning
message: vi.fn() call
rule:
  pattern: vi.fn($$$)
---
id: sinon-method
language: ts
severity: warning
message: sinon mock method call
rule:
  pattern: sinon.$M($$$)`;

// plugins/ts-quality/hooks/src/rules/ast-rules.ts
function hits(all, rule, limit) {
  return all.filter((hit) => hit.ruleId === rule).slice(0, limit).map((hit) => hit.excerpt.replace(/\t+$/, ""));
}
function group(f, all, header, rules) {
  const lines = rules.flatMap(([rule, limit]) => hits(all, rule, limit));
  return lines.length === 0 ? [] : [`${header.replace("$F", f.file.path)}
${lines.join(`
`)}`];
}
function scanFailure(scan) {
  if (scan.kind !== "failed")
    return;
  if (scan.stage === "parse")
    return "ast-grep exited 0 but its output did not parse as the documented JSON array";
  const first = scan.stderrFirst === "" ? "" : `: ${scan.stderrFirst}`;
  return `ast-grep exit ${String(scan.exitCode)}${first}`;
}
function errorHandling(f) {
  const scan = astGrepScan(f.file, ERROR_RULES, f.ctx);
  if (scan.kind === "missing")
    return [];
  const all = scan.kind === "ok" ? scan.hits : [];
  const errors = [
    ...group(f, all, "Empty catch block in $F \u2014 handle the error or rethrow; do not swallow", [
      ["empty-catch", 3],
      ["empty-catch-noarg", 3]
    ]),
    ...group(f, all, "Silent promise rejection in $F \u2014 log or rethrow the error", [
      ["empty-catch-handler", 3],
      ["null-catch-handler", 3],
      ["undef-catch-handler", 3]
    ]),
    ...group(f, all, "Catch swallows the error by returning a nullish value in $F \u2014 handle, log, or rethrow it", [
      ["swallow-null-arg", 2],
      ["swallow-undef-arg", 2],
      ["swallow-null", 2],
      ["swallow-undef", 2],
      ["swallow-bare", 2]
    ]),
    ...group(f, all, "throw new Error() with no message in $F \u2014 include a descriptive message", [
      ["throw-empty-error", 3]
    ]),
    ...group(f, all, "throw of string literal in $F \u2014 throw an Error (or subclass) instead", [
      ["throw-string", 3],
      ["throw-template", 3]
    ])
  ];
  const failure = scanFailure(scan);
  if (failure !== undefined) {
    errors.push(`ast-grep failed while scanning ${f.file.path} \u2014 ${failure}; error-handling rules could not be verified. Fix the tool/file and re-edit`);
  }
  return errors;
}
function mockScanFailure(f, scan) {
  const at = `ast-grep failed while scanning ${f.file.path} for mocks \u2014`;
  const tail = "no-mocks rule could not be verified. Fix the tool/file and re-edit";
  if (scan.kind === "ok" && scan.empty) {
    return `${at} exited 0 with empty output (expected at least the JSON array "[]"); ${tail}`;
  }
  if (scan.kind !== "failed")
    return;
  if (scan.stage === "parse")
    return `${at} its output did not parse as the documented JSON array; ${tail}`;
  const first = scan.stderrFirst === "" ? "" : `: ${scan.stderrFirst}`;
  return `${at} exit ${String(scan.exitCode)}${first}; ${tail}`;
}
function mockDoubles(f) {
  const path = f.file.path;
  const inTests = isTestPath(path) || path.includes("/__tests__/");
  if (path.includes("/e2e/") || !inTests || !f.limits.noMocks)
    return [];
  const errors = [];
  const scan = astGrepScan(f.file, MOCK_RULES, f.ctx);
  const failure = scan.kind === "missing" ? undefined : mockScanFailure(f, scan);
  if (failure !== undefined)
    errors.push(failure);
  const found = scan.kind === "ok" ? scan.hits.toSorted((a, b) => a.line - b.line) : [];
  if (found.length > 0) {
    const excerpt = found.slice(0, 5).map((hit) => hit.excerpt).join(`
`);
    errors.push(`Mocked test double in ${path} \u2014 tests must exercise real data/services, not mocks/stubs (jest.mock/vi.mock/jest.fn/vi.fn/sinon)
${excerpt}`);
  }
  if (f.lines.some((line) => ere(`from[[:space:]]+["']ts-mockito["']`).test(line))) {
    errors.push(`Import from ts-mockito in ${path} \u2014 tests must exercise real data/services, not mocks/stubs`);
  }
  return errors;
}

// plugins/ts-quality/hooks/src/rules/layout-rules.ts
import { lstatSync, readdirSync as readdirSync3, readFileSync as readFileSync5, statSync as statSync7 } from "fs";
import { basename as basename3, dirname as dirname4, join as join9, resolve as resolve2 } from "path";
function declaresAtAlias(config) {
  let doc;
  try {
    doc = JSON.parse(readFileSync5(config, "utf8"));
  } catch {
    return false;
  }
  if (doc !== null && !isJsonObject(doc))
    return false;
  const options = doc === null ? null : doc.compilerOptions ?? null;
  if (options !== null && !isJsonObject(options))
    return false;
  const paths = options === null ? null : options.paths ?? null;
  if (paths === null || paths === false)
    return false;
  return isJsonObject(paths) && Object.keys(paths).some((key) => key.startsWith("@/"));
}
function isFile2(path) {
  try {
    return statSync7(path).isFile();
  } catch {
    return false;
  }
}
function hasAtAlias(f) {
  const root = resolve2(f.ctx.projectRoot);
  let dir = dirname4(f.file.absolute);
  for (;; ) {
    for (const name of ["tsconfig.json", "tsconfig.base.json"]) {
      const config = join9(dir, name);
      if (isFile2(config) && declaresAtAlias(config))
        return true;
    }
    if (dir === root || dir === "/")
      return false;
    dir = dirname4(dir);
  }
}
function parentImport(f) {
  if (!f.lines.some((line) => /from ["']\.\.\//s.test(line)))
    return;
  if (!hasAtAlias(f))
    return;
  return `Forbidden ../ import in ${f.file.path} \u2014 use @/ alias`;
}
function entries2(dir) {
  try {
    return readdirSync3(dir);
  } catch {
    return [];
  }
}
function kind(path) {
  try {
    const stat = lstatSync(path);
    return stat.isFile() ? "file" : stat.isDirectory() ? "dir" : "other";
  } catch {
    return "other";
  }
}
function hasSource(dir) {
  return entries2(dir).some((name) => /\.tsx?$/s.test(name) && !name.includes(".test.") && !name.includes(".spec.") && !name.endsWith(".d.ts") && kind(join9(dir, name)) === "file");
}
function hasOtherDir(dir, absolute) {
  const own = dir === "/" ? "/" : basename3(dir);
  if (kind(absolute) === "dir" && own !== "__tests__" && own !== ".")
    return true;
  return entries2(absolute).some((name) => name !== "__tests__" && kind(join9(absolute, name)) === "dir");
}
function testPlacement(f) {
  const path = f.file.path;
  if (!isTestPath(path) || path.includes("/e2e/"))
    return [];
  if (!path.includes("/__tests__/")) {
    return [`Test file outside __tests__/: ${path} \u2014 move to sibling __tests__/ directory`];
  }
  const errors = [];
  const cut = path.lastIndexOf("__tests__/");
  const after = path.slice(cut + "__tests__/".length);
  const sub = after.includes("/") ? after.slice(0, after.indexOf("/")) : undefined;
  if (sub !== undefined && sub !== "fixtures" && sub !== "helpers" && sub !== "utils") {
    errors.push(`Test nested in __tests__/ subdirectory: ${path} \u2014 keep __tests__/ flat (only fixtures/helpers/utils subdirs allowed; no mocks/ \u2014 tests must exercise real data)`);
  }
  const parent = dirname4(`${path.slice(0, cut)}__tests__`);
  const absolute = resolve2(f.ctx.cwd ?? process.cwd(), parent);
  if (!hasSource(absolute) && !hasOtherDir(parent, absolute)) {
    errors.push(`Test not co-located with source: ${path} \u2014 __tests__/ must be at the same level as the code it tests`);
  }
  return errors;
}

// plugins/ts-quality/hooks/src/rules/line-rules.ts
import { readFileSync as readFileSync6, statSync as statSync8 } from "fs";
import { basename as basename4, join as join10 } from "path";
var AS_PATTERN = ere(String.raw`\)[[:space:]]+as[[:space:]]+[a-zA-Z]|\bas[[:space:]]+any\b|\bas[[:space:]]+unknown\b|[a-zA-Z>][[:space:]]+as[[:space:]]+[A-Z]|[a-zA-Z>][[:space:]]+as[[:space:]]+(string|number|boolean|object|symbol|bigint|never|undefined|null|void)\b`);
var AS_CONST = ere(String.raw`\bas[[:space:]]+const\b`);
var AS_IMPORT_OR_REEXPORT = ere(String.raw`\bimport\b|^[0-9]+:[[:space:]]*export[[:space:]]*(type[[:space:]]+)?\{`);
function typeAssertion(f) {
  const rows = withoutCommentLines(numbered(f.lines, AS_PATTERN)).filter((row) => !AS_CONST.test(row) && !AS_IMPORT_OR_REEXPORT.test(row));
  const header = `Forbidden 'as' type assertion in ${f.file.path} \u2014 use type guards or Zod`;
  return withExcerpt(header, head(rows, 5));
}
function reactHooks(f) {
  if (!/use-.*\.ts$/s.test(f.file.path) && !/use[A-Z].*\.ts$/s.test(f.file.path))
    return;
  const count = countMatches(f.lines, ere(String.raw`^[[:space:]]*(const \[|useRef\(|useEffect\()`));
  if (count <= 3)
    return;
  return `Hook does too many things in ${f.file.path} (${String(count)} useState/useRef/useEffect) \u2014 split into focused hooks`;
}
function factories(f) {
  const count = countMatches(f.lines, /^export (async )?function create/s);
  if (count <= 2)
    return;
  return `Too many factory functions in ${f.file.path} (${String(count)}) \u2014 simplify construction`;
}
function projectUsesZod(f) {
  const manifest = join10(f.ctx.projectRoot, "package.json");
  try {
    return statSync8(manifest).isFile() && readFileSync6(manifest, "utf8").includes('"zod"');
  } catch {
    return false;
  }
}
function manualTypeGuard(f) {
  if (countMatches(f.lines, /function is[A-Z].*\): .* is [A-Z]/s) === 0)
    return;
  if (!projectUsesZod(f))
    return;
  return `Manual type guard in ${f.file.path} \u2014 use Zod schema instead`;
}
function componentFileName(f) {
  if (!/\.(tsx)$/s.test(f.file.path))
    return;
  const name = basename4(f.file.path).replace(/\.tsx$/s, "").replace(/\.ts$/s, "");
  const grabBag = /^(parts|components|helpers|items|sections|elements)$|-(parts|sections|items|elements)$/s;
  if (!grabBag.test(name))
    return;
  return `Forbidden component filename '${name}.tsx' in ${f.file.path} \u2014 name file after its exported function (e.g. api-key-create-button.tsx)`;
}
function consoleLog(f) {
  const rows = withoutCommentLines(numbered(f.lines, ere(String.raw`^[[:space:]]*console\.log\(`)));
  const header = `Forbidden console.log in ${f.file.path} \u2014 use console.error/warn/info`;
  return withExcerpt(header, head(rows, 3));
}
function suppressionComment(f) {
  const tokens = isTestPath(f.file.path) ? "@ts-ignore|@ts-nocheck|eslint-disable|biome-ignore" : "@ts-ignore|@ts-nocheck|eslint-disable|biome-ignore|@ts-expect-error";
  const rows = numbered(f.lines, ere(String.raw`(//|/\*+)[[:space:]]*(${tokens})`));
  const header = `Forbidden suppression comment in ${f.file.path} \u2014 fix the underlying issue in code, never silence it`;
  return withExcerpt(header, head(rows, 3));
}
function isFrontend(f) {
  return pathHas(f, "/components/") || pathHas(f, "/routes/");
}
function confirmAlert(f) {
  if (!isFrontend(f))
    return;
  const shared = /(ConfirmDeleteAlert|AlertDialog|customAlert|customConfirm)/s;
  const rows = withoutCommentLines(numbered(f.lines, ere(String.raw`\b(confirm|alert)[[:space:]]*\(`))).filter((row) => !shared.test(row));
  const header = `Forbidden confirm()/alert() in ${f.file.path} \u2014 use AlertDialog component`;
  return withExcerpt(header, head(rows, 3));
}
function rawRadixImport(f) {
  const imports = numbered(f.lines, /from ['"]@radix-ui\/react-(alert-dialog|dialog)['"]/s);
  if (withoutCommentLines(imports).length === 0 || pathHas(f, "/packages/ui/"))
    return;
  return `Raw radix import in ${f.file.path} \u2014 use shared components from @/components/ui/`;
}
function mutableProps(f) {
  const rows = numbered(f.lines, ere(String.raw`\((props|[a-z]+Props):[[:space:]]+[A-Z][a-zA-Z]+Props\)`)).filter((row) => !row.includes("Readonly"));
  return withExcerpt(`Mutable props in ${f.file.path} \u2014 wrap in Readonly<Props>`, head(rows, 3));
}
function catchToast(f) {
  if (!isFrontend(f))
    return;
  const opens = ere(String.raw`catch[[:space:]]*\(`);
  const rows = [];
  let found = false;
  f.lines.forEach((line, i) => {
    if (opens.test(line))
      found = true;
    if (found && line.includes("toast(")) {
      rows.push(`${String(i + 1)}: ${line}`);
      found = false;
    }
  });
  const header = `Manual try/catch+toast in ${f.file.path} \u2014 use shared error handling`;
  return withExcerpt(header, head(rows, 3));
}

// plugins/ts-quality/hooks/src/rules/size-rules.ts
function fileTooLong(f) {
  const max = f.limits.fileLines;
  const count = countCodeLines(f.file.absolute) ?? 0;
  if (count <= max)
    return;
  let hint = "split into smaller modules";
  const linter = tsLinter({
    env: f.ctx.env,
    ...f.ctx.cwd === undefined ? {} : { cwd: f.ctx.cwd }
  });
  if (linter !== undefined && f.limits.fileSource === "native") {
    hint += ` (${linter} enforces this max-lines limit)`;
  } else if (linter === "biome" && f.limits.fileSource === "default") {
    hint += ` (biome has no max-lines equivalent \u2014 gate uses the ${String(max)}-line default)`;
  } else if (linter !== undefined && f.limits.fileSource === "default") {
    hint += ` (${linter} is present but the gate's limit didn't come from its config (unparsed config form or a per-glob override) \u2014 gate uses the ${String(max)}-line default; align them)`;
  }
  const approx = hasUnterminatedBlock(f.file.absolute) ? " (size approximated \u2014 an unterminated /* or a string containing /* may be affecting the count)" : "";
  return `TS file exceeds ${String(max)}-line limit: ${f.file.path} (${String(count)} code lines, blanks/comments excluded)${approx} \u2014 ${hint}`;
}
function strip(line) {
  return line.replaceAll("\\\"", "").replaceAll(/"[^"]*"/g, "").replaceAll(/'[^']*'/g, "").replaceAll(/`[^`]*`/g, "");
}
var FUNCTION_DECL = ere(String.raw`^[[:space:]]*(export[[:space:]]+)?(default[[:space:]]+)?(async[[:space:]]+)?function[ \t*]`);
var CONST_FN = ere(String.raw`^[[:space:]]*(export[[:space:]]+)?(default[[:space:]]+)?const[[:space:]]+[A-Za-z_$][A-Za-z0-9_$]*[[:space:]]*=[[:space:]]*(async[[:space:]]+)?(\(|function[ \t(*])`);
var METHOD_HEAD = ere(String.raw`^[[:space:]]+(public[[:space:]]+|private[[:space:]]+|protected[[:space:]]+|static[[:space:]]+|async[[:space:]]+|override[[:space:]]+|readonly[[:space:]]+|get[[:space:]]+|set[[:space:]]+|\*[[:space:]]*)*[A-Za-z_$][A-Za-z0-9_$]*[[:space:]]*(<[^(){}]*>)?[[:space:]]*\(`);
var KEYWORD_HEAD = ere(String.raw`^[[:space:]]*(if|for|while|switch|catch|return|do|else|function|await|with|yield|throw|new|typeof|delete|void|in|of|case)[^A-Za-z0-9_$]`);
var METHOD_BODY = ere(String.raw`\)([[:space:]]*:[^={]*)?[[:space:]]*\{[[:space:]]*$`);
var ENDS_STATEMENT = ere(String.raw`;[[:space:]]*$`);
function startsFunction(line) {
  const s = strip(line);
  if (FUNCTION_DECL.test(s) || CONST_FN.test(s))
    return true;
  return METHOD_HEAD.test(s) && !KEYWORD_HEAD.test(s) && !s.includes("=>") && !ENDS_STATEMENT.test(s) && METHOD_BODY.test(s);
}
function count(text, char) {
  return text.split(char).length - 1;
}
function anyLongFunction(lines, max) {
  let open;
  let long = false;
  lines.forEach((line, i) => {
    if (open === undefined && startsFunction(line))
      open = { start: i, depth: 0, opened: false };
    if (open === undefined)
      return;
    const s = strip(line);
    const opens = count(s, "{");
    open.depth += opens - count(s, "}");
    if (opens > 0)
      open.opened = true;
    if (open.opened && open.depth <= 0) {
      if (i - open.start > max)
        long = true;
      open = undefined;
      return;
    }
    if (!open.opened && ENDS_STATEMENT.test(line))
      open = undefined;
  });
  return long;
}
function functionTooLong(f) {
  if (!anyLongFunction(f.lines, f.limits.fnLines))
    return;
  return `Function too long in ${f.file.path} (>${String(f.limits.fnLines)} lines) \u2014 simplify or split`;
}

// plugins/ts-quality/hooks/src/rules/type-rules.ts
import { spawnSync as spawnSync6 } from "child_process";
var PATHSPECS = ["packages/*.ts", "packages/*.tsx", "apps/*.ts", "apps/*.tsx"];
function definedElsewhere(f, name, relative) {
  const pattern = `^export (interface|type) ${name}[ <{]`;
  const res = spawnSync6("git", ["-C", f.ctx.projectRoot, "grep", "-l", "--untracked", "-E", pattern, "--", ...PATHSPECS], { cwd: f.ctx.cwd, env: spawnEnv(f), encoding: "utf8" });
  const files = (res.stdout ?? "").split(`
`).filter((line) => line !== "" && line !== relative);
  return files[0] ?? "";
}
function duplicateTypes(f) {
  const names = f.lines.flatMap((line) => {
    const match = /^export (?:interface|type) ([A-Z][a-zA-Z]+)/s.exec(line);
    return match?.[1] === undefined ? [] : [match[1]];
  });
  if (names.length === 0)
    return [];
  const prefix = `${f.ctx.projectRoot}/`;
  const path = f.file.path;
  const relative = path.startsWith(prefix) ? path.slice(prefix.length) : path;
  return names.flatMap((name) => {
    const other = definedElsewhere(f, name, relative);
    return other === "" ? [] : [`Type '${name}' in ${path} already defined in ${other} \u2014 import instead of redefining`];
  });
}
var THROW_LITERAL = /(^|[^a-zA-Z_$])throw[ \t]+(-?[0-9]+(\.[0-9]+)?|null|undefined|true|false)([ \t]|;|}|$)/s;
function throwLiteral(f) {
  const rows = f.lines.flatMap((line, i) => {
    const code = line.replaceAll(/\/\*.*\*\//gs, "").replace(/\/\/.*$/s, "");
    return THROW_LITERAL.test(code) ? [`${String(i + 1)}: ${line}`] : [];
  });
  const excerpt = head(rows, 3);
  if (excerpt === "")
    return;
  return `throw of non-Error literal in ${f.file.path} \u2014 throw an Error (or subclass) instead
${excerpt}`;
}

// plugins/ts-quality/hooks/src/rules/check.ts
var RULES = [
  parentImport,
  typeAssertion,
  testPlacement,
  fileTooLong,
  functionTooLong,
  reactHooks,
  factories,
  manualTypeGuard,
  duplicateTypes,
  componentFileName,
  consoleLog,
  suppressionComment,
  confirmAlert,
  rawRadixImport,
  mutableProps,
  catchToast,
  errorHandling,
  throwLiteral,
  mockDoubles
];
function checkTsFile(f) {
  const errors = RULES.flatMap((rule) => rule(f) ?? []);
  const duplicated = errors.length === 0 ? duplication(f) : "";
  return { errors, advisories: [duplicated, docs(f), unhandledAwait(f)] };
}

// plugins/ts-quality/hooks/src/post-tool-use.ts
var ALLOW2 = { kind: "allow" };
function limitsFor(ctx) {
  const where = { env: ctx.env, ...ctx.cwd === undefined ? {} : { cwd: ctx.cwd } };
  const config = loadConfig({ ...where, host: ctx.host, warn: () => {
    return;
  } });
  const file = tsMaxFileLinesResolved(config, where);
  return {
    fileLines: file.value,
    fileSource: file.source,
    fnLines: qualityThreshold(config, "ts", "maxFnLines", where),
    noMocks: qualityFlag(config, "ts", "noMocks", true)
  };
}
function read(file) {
  try {
    return { text: readFileSync7(file.absolute, "utf8") };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return { error: `Cannot read ${file.path} for TypeScript quality checks: ${detail}` };
  }
}
var post_tool_use_default = defineRegistryModule({
  spec: "ts-quality@toolu",
  name: "ts-quality",
  event: "tool/post",
  run(event, ctx) {
    const where = { env: ctx.env, ...ctx.cwd === undefined ? {} : { cwd: ctx.cwd } };
    if (!detectTs(where))
      return Promise.resolve(ALLOW2);
    const pm = nodePackageManager(where);
    if (pm === undefined || !toolAvailable(pm, ctx.env))
      return Promise.resolve(ALLOW2);
    const decision = fileQuality(event, ctx, {
      source: "ts-quality-hook",
      reason: "Post-edit quality violation(s) detected",
      matches: /\.(ts|tsx)$/s,
      skipLinkedWorktrees: true,
      check: (file) => {
        const source = read(file);
        if (source.error !== undefined)
          return { errors: [source.error], advisories: [] };
        return checkTsFile({ file, lines: splitLines(source.text), ctx, limits: limitsFor(ctx), pm });
      }
    });
    return Promise.resolve(decision);
  }
});
export {
  post_tool_use_default as default
};
