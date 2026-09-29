/**
 * Per-step freshness scopes (#256): `pl_scope_sha` and `pl_scope_map`. A step
 * that declares `paths` is judged on the hash of `git diff BASE...HEAD --
 * <paths>`, with the declaration itself hashed in. An edit outside those
 * paths then leaves it fresh, and editing the declaration invalidates it.
 */
import { childEnv, type HostEnv } from "../host/host-name.ts";
import { commandSubstitution } from "./ledger-check.ts";
import { eachOptional, get, raw, type Json, type JsonObject } from "./ledger-jq.ts";

/**
 * `pl_scope_sha BASE PATHS`: the hash of `<path>\0...` plus the scoped diff,
 * or undefined when `git diff` or `git hash-object` fails (the caller then
 * falls back to the branch hash). `cwd` is the project root.
 */
export function scopeSha(
  base: string,
  paths: readonly string[],
  cwd: string,
  env: HostEnv,
): string | undefined {
  if (paths.length === 0) return undefined;
  const spawnEnv = childEnv(env);
  const diff = Bun.spawnSync(["git", "diff", "--no-color", `${base}...HEAD`, "--", ...paths], {
    cwd,
    env: spawnEnv,
    stdout: "pipe",
    stderr: "ignore",
  });
  if (!diff.success) return undefined;
  const declared = new TextEncoder().encode(paths.map((p) => `${p}\0`).join(""));
  const body = commandSubstitution(diff.stdout);
  const input = new Uint8Array(declared.length + body.length);
  input.set(declared, 0);
  input.set(body, declared.length);
  const hash = Bun.spawnSync(["git", "hash-object", "--stdin"], {
    cwd,
    env: spawnEnv,
    stdin: input,
    stdout: "pipe",
    stderr: "ignore",
  });
  const sha = hash.success ? hash.stdout.toString("utf8").trim() : "";
  return sha === "" ? undefined : sha;
}

/**
 * `pl_scope_map STEPS BASE`: `{id: scope sha}` for every step that declares
 * non-empty `paths`. A step whose scope cannot be hashed is left out, and a
 * warning is emitted so the fallback to the branch-wide rule is visible.
 */
export function scopeMap(
  steps: readonly Json[],
  base: string,
  cwd: string,
  env: HostEnv,
  warn: (line: string) => void,
): JsonObject {
  const out: JsonObject = {};
  for (const step of steps) {
    const paths = get(step, "paths");
    if (!Array.isArray(paths) || paths.length === 0) continue;
    const id = raw(get(step, "id"));
    if (id === "") continue;
    const sha = scopeSha(base, eachOptional(paths).map(raw), cwd, env);
    if (sha === undefined) {
      warn(
        `plan-ledger: step ${id} declares paths that could not be hashed; judging it on the whole branch diff`,
      );
      continue;
    }
    out[id] = sha;
  }
  return out;
}
