/**
 * toolu's entry as the lowest config layer (#345). OpenCode deep-merges config
 * sources with the later source winning per key, so a user's entry is merged
 * over toolu's: plain objects merge key by key, and arrays and scalars come
 * from the user. Key order follows the host's own merge (base keys first, then
 * keys only the user has), which matters for permission rules evaluated last
 * match wins.
 */
export function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const proto: unknown = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

export function mergeUnder(
  base: Record<string, unknown>,
  over: Record<string, unknown>,
): Record<string, unknown> {
  const out: Record<string, unknown> = { ...base };
  for (const [key, value] of Object.entries(over)) {
    const current = out[key];
    out[key] = isPlainRecord(current) && isPlainRecord(value) ? mergeUnder(current, value) : value;
  }
  return out;
}
