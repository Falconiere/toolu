/**
 * Push-review waivers (#256), a port of `push-waiver.sh`: "yes, push anyway",
 * remembered for exactly one diff. The push-review gate asks and records a
 * pending marker naming the diff sha (`pushWaiverPend`). A successful push
 * of that same sha promotes it to a waiver (`pushWaiverPromote`). The next
 * push of that diff finds it (`pushWaiverMatches`). A new commit changes the
 * sha, so the waiver stops matching, and the state sweeper reclaims stale
 * files. Writes are atomic and 0600, and the bytes match bash's `jq -c` lines.
 */
import { mkdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { dirname } from "node:path";
import { envValue, type HostEnv, type HostName } from "../host/host-name.ts";
import { projectStateDir, resolveHost } from "../host/host-roots.ts";
import { isoSeconds, toJqJson, writeAtomic } from "../state/state-io.ts";
import { JqError, alt, get, isObject, parseJson, raw, type JsonObject } from "./ledger-jq.ts";

export const PUSH_WAIVER_VERSION = 1;

export type WaiverOptions = { env?: HostEnv; host?: HostName; now?: () => Date };

/** A waiver or pending marker as written (`asked_at` on a pending marker, `waived_at` on a waiver). */
export type WaiverFile = {
  version: 1;
  branch: string;
  diff_sha: string;
  base_branch: string;
  reason_code: string;
  asked_at?: string;
  waived_at?: string;
};

/** `push_waiver_dir ROOT`: `$STATE_DIR`, else the push-review state dir of ROOT (the project root when empty). */
export function pushWaiverDir(root: string, options: WaiverOptions = {}): string {
  const o = resolveHost({
    env: options.env ?? process.env,
    ...(options.host === undefined ? {} : { host: options.host }),
  });
  const override = envValue(o.env, "STATE_DIR");
  if (override !== undefined) return override;
  return projectStateDir("push-review", root === "" ? o : { ...o, root }) ?? "";
}

export function pushWaiverPath(root: string, slug: string, options: WaiverOptions = {}): string {
  return `${pushWaiverDir(root, options)}/${slug}.waiver.json`;
}

export function pushWaiverPendingPath(
  root: string,
  slug: string,
  options: WaiverOptions = {},
): string {
  return `${pushWaiverDir(root, options)}/${slug}.pending-waiver.json`;
}

function readObject(file: string): JsonObject | undefined {
  try {
    if (!statSync(file).isFile()) return undefined;
    const value = parseJson(readFileSync(file, "utf8"));
    return isObject(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

/**
 * `_push_waiver_sha FILE`: the diff sha of a v1 waiver-shaped file. It is
 * undefined for a missing file, unreadable JSON, an unknown version or an
 * empty sha; all of these must read as "no waiver", never as a match.
 */
function waiverSha(file: string): string | undefined {
  const doc = readObject(file);
  if (doc === undefined) return undefined;
  try {
    if (raw(alt(get(doc, "version"), "")) !== String(PUSH_WAIVER_VERSION)) return undefined;
    const sha = raw(alt(get(doc, "diff_sha"), ""));
    return sha === "" ? undefined : sha;
  } catch (error) {
    if (error instanceof JqError) return undefined;
    throw error;
  }
}

function write(file: string, doc: JsonObject): boolean {
  try {
    mkdirSync(dirname(file), { recursive: true });
  } catch {
    return false;
  }
  return writeAtomic(file, `${toJqJson(doc, false)}\n`);
}

function now(options: WaiverOptions): string {
  return isoSeconds(options.now?.() ?? new Date());
}

/** `push_waiver_matches ROOT SLUG SHA`: a waiver covers exactly SHA. */
export function pushWaiverMatches(
  root: string,
  slug: string,
  sha: string,
  options: WaiverOptions = {},
): boolean {
  return sha !== "" && waiverSha(pushWaiverPath(root, slug, options)) === sha;
}

/** `push_waiver_pend ROOT SLUG SHA BASE REASON_CODE`: record the question; the latest one wins. */
export function pushWaiverPend(
  root: string,
  slug: string,
  sha: string,
  base: string,
  reasonCode: string,
  options: WaiverOptions = {},
): boolean {
  if (sha === "") return false;
  const doc: WaiverFile = {
    version: 1,
    branch: slug,
    diff_sha: sha,
    base_branch: base,
    reason_code: reasonCode,
    asked_at: now(options),
  };
  return write(pushWaiverPendingPath(root, slug, options), doc);
}

/**
 * `push_waiver_promote ROOT SLUG SHA`: cash in the pending marker, but only
 * when it names SHA. A marker from an older diff must not waive different code.
 */
export function pushWaiverPromote(
  root: string,
  slug: string,
  sha: string,
  options: WaiverOptions = {},
): boolean {
  if (sha === "") return false;
  const pending = pushWaiverPendingPath(root, slug, options);
  if (waiverSha(pending) !== sha) return false;
  const marker = readObject(pending);
  if (marker === undefined) return false;
  const waiver: JsonObject = { ...marker };
  delete waiver.asked_at;
  waiver.waived_at = now(options);
  if (!write(pushWaiverPath(root, slug, options), waiver)) return false;
  rmSync(pending, { force: true });
  return true;
}
