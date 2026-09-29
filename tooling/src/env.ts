/** Environment overrides with shell `${NAME:-fallback}` semantics. */

/** An unset or EMPTY variable reads as `fallback`, exactly like `${NAME:-fallback}`. */
export function envOr(
  name: string,
  fallback: string,
  env: Record<string, string | undefined> = process.env,
): string {
  const value = env[name];
  return value === undefined || value === "" ? fallback : value;
}
