/**
 * The jq semantics the ledger, verdict and waiver pipelines depend on (#256).
 * The bash libs run jq over plain JSON, so a legacy or hand-edited file is
 * accepted wherever jq accepts it and fails exactly where jq raises a type
 * error. These helpers reproduce that: `.key` and `.[]` throw `JqError` on the
 * shapes jq rejects, `//` treats `false` like `null`, and `raw` renders a value
 * the way `jq -r` prints it.
 */
import { toJqJson } from "../state/state-io.ts";

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type JsonObject = { [key: string]: Json };

/** A jq runtime error: the bash caller's `|| { echo "..."; return 2; }` path. */
export class JqError extends Error {
  override readonly name = "JqError";
}

/** jq's `type`. */
export function jqType(value: Json): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value === "object" ? "object" : typeof value;
}

export function isObject(value: Json | undefined): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** jq `.[key]`: null through null, a missing key is null, any other input is an error. */
export function get(value: Json, key: Json): Json {
  if (value === null) return null;
  if (isObject(value) && typeof key === "string") {
    return Object.hasOwn(value, key) ? (value[key] ?? null) : null;
  }
  throw new JqError(`Cannot index ${jqType(value)} with ${jqType(key)}`);
}

/** jq `.[]`: array items or object values; anything else (null included) is an error. */
export function each(value: Json): Json[] {
  if (Array.isArray(value)) return value;
  if (isObject(value)) return Object.values(value);
  throw new JqError(`Cannot iterate over ${jqType(value)}`);
}

/** jq `.[]?`: like `each`, but an un-iterable input yields nothing. */
export function eachOptional(value: Json): Json[] {
  return Array.isArray(value) || isObject(value) ? each(value) : [];
}

/** jq `a // b`: `null`, `false` and absence all fall through to the fallback. */
export function alt(value: Json | undefined, fallback: Json): Json {
  return value === undefined || value === null || value === false ? fallback : value;
}

/** jq truthiness: everything but `null` and `false`. */
export function truthy(value: Json | undefined): boolean {
  return value !== undefined && value !== null && value !== false;
}

/** jq `==`: structural equality (object key order is irrelevant). */
export function jqEquals(a: Json, b: Json): boolean {
  if (a === b) return true;
  if (Array.isArray(a) || Array.isArray(b)) {
    return (
      Array.isArray(a) &&
      Array.isArray(b) &&
      a.length === b.length &&
      a.every((item, i) => jqEquals(item, b[i] ?? null))
    );
  }
  if (isObject(a) && isObject(b)) {
    const keys = Object.keys(a);
    return (
      keys.length === Object.keys(b).length &&
      keys.every((key) => Object.hasOwn(b, key) && jqEquals(a[key] ?? null, b[key] ?? null))
    );
  }
  return false;
}

/** jq `length`. */
export function length(value: Json): number {
  if (value === null) return 0;
  if (typeof value === "boolean") throw new JqError("boolean has no length");
  if (typeof value === "number") return Math.abs(value);
  if (typeof value === "string") return value.match(/[\s\S]/gu)?.length ?? 0;
  return Array.isArray(value) ? value.length : Object.keys(value).length;
}

/**
 * jq `index($x)` for a string `$x`: the first position in an array or string,
 * or null when absent. On an object it is the first element of `.[$x]`. A null
 * input gives null, and jq raises on any other input.
 */
export function jqIndex(container: Json, needle: string): Json {
  if (container === null) return null;
  if (Array.isArray(container)) {
    const at = container.findIndex((item) => jqEquals(item, needle));
    return at === -1 ? null : at;
  }
  if (typeof container === "string") {
    const at = container.indexOf(needle);
    return at === -1 ? null : at;
  }
  const found = get(container, needle);
  if (found === null) return null;
  if (Array.isArray(found)) return found[0] ?? null;
  throw new JqError(`Cannot index ${jqType(found)} with number`);
}

/** `select(X | index($x))`: jq truthiness of `jqIndex`. */
export function holds(container: Json, needle: string): boolean {
  return truthy(jqIndex(container, needle));
}

/** jq `tostring`: strings verbatim, everything else as compact JSON. */
export function toStr(value: Json): string {
  return typeof value === "string" ? value : toJqJson(value, false);
}

/** One value as `jq -r` prints it: strings raw, everything else as pretty JSON. */
export function raw(value: Json): string {
  return typeof value === "string" ? value : toJqJson(value, true);
}

/** jq `"a" + x` for a string on the left: only a string (or null) may follow. */
export function concat(...parts: Json[]): string {
  let out = "";
  for (const part of parts) {
    if (part === null) continue;
    if (typeof part !== "string") throw new JqError(`string and ${jqType(part)} cannot be added`);
    out += part;
  }
  return out;
}

/** Narrow a `JSON.parse` result, which is always one of the JSON shapes. */
function isJson(value: unknown): value is Json {
  if (value === null || ["string", "number", "boolean"].includes(typeof value)) return true;
  if (Array.isArray(value)) return value.every(isJson);
  return typeof value === "object" && Object.values(value).every(isJson);
}

/** `JSON.parse` that reports failure as undefined rather than throwing. */
export function parseJson(text: string): Json | undefined {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return undefined;
  }
  return isJson(value) ? value : undefined;
}
