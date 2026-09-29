/** Reads over untyped JSON (CLI output, manifests) without type assertions. */

/** `value.k1.k2…`, undefined when any step is missing or not an object. */
export function get(value: unknown, ...keys: Array<string | number>): unknown {
  let current = value;
  for (const key of keys) {
    current =
      typeof current === "object" && current !== null ? Reflect.get(current, key) : undefined;
  }
  return current;
}

export function list(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

export function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function isNullish(value: unknown): boolean {
  return value === undefined || value === null;
}
