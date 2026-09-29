/**
 * One-time host permission allowlist (#253): port of
 * `plugins/toolu/hooks/lib/permissions.sh`. The first toolu session in a Claude
 * repo unions an allowlist into `<project>/.claude/settings.local.json` and
 * records a sentinel under the git-ignored state root, so a rule the user
 * deletes stays deleted. A settings file that cannot be parsed is left
 * byte-identical. An invalid config envelope skips the write (fail closed:
 * never broaden grants from a config toolu cannot read).
 */
import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { childEnv, type HostEnv } from "../host/host-name.ts";
import { projectDirname, projectRoot, projectStateRoot } from "../host/host-roots.ts";
import { isJsonObject, type JsonObject, type LoadedConfig } from "./config-load.ts";
import { flagFalse, section } from "./config-read.ts";

/** Blanket shell plus the two edit tools; `Bash(*)` subsumes `Bash(git:*)`. */
export const DEFAULT_PERMISSIONS: readonly string[] = ["Bash(*)", "Edit", "Write"];
export const PERMISSIONS_SENTINEL = ".permissions-written";

export type PermissionsOptions = { env?: HostEnv; cwd?: string };
export type PermissionsResult =
  | { written: true; settingsFile: string; added: string[]; notice: string | null }
  | { written: false; reason: string };

/** A configured `permissions.allow` (its strings) replaces the default outright. */
function entries(config: LoadedConfig): readonly string[] {
  const allow = section(config, "permissions")?.allow;
  const configured = Array.isArray(allow)
    ? allow.filter((item): item is string => typeof item === "string")
    : [];
  return configured.length > 0 ? configured : DEFAULT_PERMISSIONS;
}

function isGitRepo(root: string, env: HostEnv): boolean {
  const res = spawnSync("git", ["-C", root, "rev-parse", "--git-dir"], {
    env: childEnv(env),
    encoding: "utf8",
  });
  return res.error === undefined && res.status === 0;
}

type Existing = { ok: true; settings: JsonObject } | { ok: false; reason: string };

/** `jq -e .` then the merge filter's own type errors (non-object, unmappable allow). */
function readSettings(file: string): Existing {
  if (!existsSync(file)) {
    return { ok: true, settings: {} };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return { ok: false, reason: `malformed JSON in ${file}; leaving it untouched` };
  }
  if (parsed === null || parsed === false) {
    return { ok: false, reason: `malformed JSON in ${file}; leaving it untouched` };
  }
  return isJsonObject(parsed)
    ? { ok: true, settings: parsed }
    : { ok: false, reason: `could not merge permissions into ${file}` };
}

/** jq `(.permissions.allow // []) | map(select(type == "string"))`; undefined on a jq error. */
function existingAllow(settings: JsonObject): string[] | undefined {
  const permissions = settings.permissions;
  if (permissions === undefined || permissions === null || permissions === false) {
    return [];
  }
  if (!isJsonObject(permissions)) {
    return undefined;
  }
  const allow = permissions.allow;
  if (allow === undefined || allow === null || allow === false) {
    return [];
  }
  const items = Array.isArray(allow)
    ? allow
    : isJsonObject(allow)
      ? Object.values(allow)
      : undefined;
  return items?.filter((item): item is string => typeof item === "string");
}

/** Temp file beside `file`, unguessable and created exclusively (`wx` refuses a planted symlink), then renamed. */
function writeAtomic(file: string, body: string): boolean {
  const tmp = `${file}.${randomUUID()}.tmp`;
  try {
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(tmp, body, { flag: "wx" });
    renameSync(tmp, file);
    return true;
  } catch {
    rmSync(tmp, { force: true });
    return false;
  }
}

type Target = { settingsFile: string; sentinel: string };

function target(
  config: LoadedConfig,
  root: string | undefined,
  options: PermissionsOptions,
): Target | string {
  const env = options.env ?? process.env;
  const scoped = {
    env,
    host: config.host,
    ...(options.cwd === undefined ? {} : { cwd: options.cwd }),
  };
  const resolved = root ?? projectRoot(scoped);
  if (resolved === undefined || resolved === "") return "no project root";
  if (config.host !== "claude") return "not claude";
  if (config.invalid !== undefined) return "config invalid";
  if (flagFalse(config, "permissions", "autoAllow")) return "autoAllow is false";
  if (!isGitRepo(resolved, env)) return "not a git repository";
  const stateRoot = projectStateRoot({ ...scoped, root: resolved });
  if (stateRoot === undefined) return "no state root";
  const sentinel = join(stateRoot, PERMISSIONS_SENTINEL);
  if (existsSync(sentinel)) return "already written";
  const settingsFile = join(resolved, projectDirname(scoped), "settings.local.json");
  return { settingsFile, sentinel };
}

/** `toolu_permissions_autowrite ROOT`. */
export function permissionsAutowrite(
  config: LoadedConfig,
  root?: string,
  options: PermissionsOptions = {},
): PermissionsResult {
  const resolved = target(config, root, options);
  if (typeof resolved === "string") {
    return { written: false, reason: resolved };
  }
  const existing = readSettings(resolved.settingsFile);
  const have = existing.ok ? existingAllow(existing.settings) : undefined;
  if (!existing.ok || have === undefined) {
    const reason = existing.ok
      ? `could not merge permissions into ${resolved.settingsFile}`
      : existing.reason;
    config.warn(reason);
    return { written: false, reason };
  }
  const added = entries(config).filter((entry) => !have.includes(entry));
  const permissions = existing.settings.permissions;
  const merged = {
    ...existing.settings,
    permissions: { ...(isJsonObject(permissions) ? permissions : {}), allow: [...have, ...added] },
  };
  if (!writeAtomic(resolved.settingsFile, `${JSON.stringify(merged, null, 2)}\n`)) {
    config.warn(`could not write ${resolved.settingsFile}`);
    return { written: false, reason: `could not write ${resolved.settingsFile}` };
  }
  // Written only after the settings write landed, so a failure retries next session.
  // Best effort, as in bash: a missing sentinel only means the union runs again.
  try {
    mkdirSync(dirname(resolved.sentinel), { recursive: true });
    writeFileSync(resolved.sentinel, "");
  } catch (error) {
    config.warn(`could not write ${resolved.sentinel}: ${String(error)}`);
  }
  const notice =
    added.length === 0
      ? null
      : `toolu wrote ${added.join(", ")} to ${resolved.settingsFile} (one time only; delete a rule and it stays deleted). Add that file to .gitignore if it is not there already.`;
  return { written: true, settingsFile: resolved.settingsFile, added, notice };
}
