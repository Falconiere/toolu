/**
 * The text bash got from `$(jq -r …)`, which the ported modules must reproduce
 * byte for byte: a string as is, any other value as jq prints it (pretty JSON),
 * trailing newlines removed by the command substitution.
 */

/** jq's `tostring`: a string as is, anything else as compact JSON. */
export function jqToString(value: unknown): string {
  return typeof value === "string" ? value : (JSON.stringify(value) ?? "null");
}

/** `$(jq -r '<expr>')` for the value `<expr>` produced. */
export function jqRaw(value: unknown): string {
  const text = typeof value === "string" ? value : (JSON.stringify(value, null, 2) ?? "null");
  return text.replace(/\n+$/u, "");
}

/** jq's `a // b`: `a` unless it is null, false or absent. */
export function jqOr(value: unknown, fallback: unknown): unknown {
  return value === undefined || value === null || value === false ? fallback : value;
}
