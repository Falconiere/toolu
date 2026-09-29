/** Host names and the environment shape every `@toolu/core/host` function reads (#252). */

export const HOST_NAMES = ["claude", "codex", "cursor", "opencode", "hermes"] as const;

export type HostName = (typeof HOST_NAMES)[number];

export type HostEnv = Readonly<Record<string, string | undefined>>;

/** `env[key]` when non-empty, else `undefined` — bash `${VAR:-}` semantics. */
export function envValue(env: HostEnv, key: string): string | undefined {
  const value = env[key];
  return value === undefined || value === "" ? undefined : value;
}

export function isHostName(value: string): value is HostName {
  return HOST_NAMES.some((host) => host === value);
}
