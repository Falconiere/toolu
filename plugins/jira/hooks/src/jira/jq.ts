/**
 * The few jq semantics the bash lean projections relied on: `.a.b` is null
 * through a missing or null parent, `.[]` iterates arrays and objects, and
 * anything else is a jq runtime error (exit 5), which ended the bash pipeline
 * with that status.
 */
import { CliExit } from "@toolu/core/cli";

export type JsonObject = Record<string, unknown>;

export function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function typeName(value: unknown): string {
  if (value === null || value === undefined) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

/** `.k1.k2…`: null through a missing or null value; indexing a non-object errors like jq. */
export function get(value: unknown, ...keys: readonly string[]): unknown {
  let current = value;
  for (const key of keys) {
    if (current === null || current === undefined) return null;
    if (!isObject(current)) {
      throw new CliExit(5, `jq: error: Cannot index ${typeName(current)} with "${key}"`);
    }
    current = Object.hasOwn(current, key) ? current[key] : null;
  }
  return current ?? null;
}

/** `.[]` over `value`: array items or object values. */
export function iterate(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  if (isObject(value)) return Object.values(value);
  throw new CliExit(5, `jq: error: Cannot iterate over ${typeName(value)}`);
}

/** `.[]` over `.key`, then `row` per item: `[.key[] | row]`. */
export function rows(value: unknown, key: string, row: (item: unknown) => unknown): unknown[] {
  return iterate(get(value, key)).map(row);
}

/** `{a, b}`: each key's value, null when absent. */
export function pick(value: unknown, ...keys: readonly string[]): JsonObject {
  return Object.fromEntries(keys.map((key) => [key, get(value, key)]));
}

/** jq's `a // b`: `b` when `a` is null, absent or false. */
export function alt(value: unknown, fallback: unknown): unknown {
  return value === null || value === undefined || value === false ? fallback : value;
}

/** How `jq -r` and string interpolation print a value: strings raw, the rest as JSON. */
export function text(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value ?? null);
}
