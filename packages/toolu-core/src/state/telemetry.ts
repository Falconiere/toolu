/**
 * Workflow telemetry append (#255), a port of `telemetry_append`. It writes
 * one line `{...extras, "v":1, "t", "branch", "event"}` (jq key order) to
 * `<root>/<host dir>/tmp/telemetry/<branch_slug>.jsonl`; `$TELEMETRY_DIR`
 * replaces the directory. It never throws. Each opt-out and each rejected
 * line is a result, not an error.
 *
 * Extras are validated against the event's closed schema (`TELEMETRY_EXTRAS`),
 * so a command line, a tool payload or a smuggled protocol key never reaches
 * the log. Bash accepted any JSON object here.
 */
import { appendFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { loadConfig } from "../config/config-load.ts";
import { enabled } from "../config/config-read.ts";
import { envValue } from "../host/host-name.ts";
import { projectStateDir } from "../host/host-roots.ts";
import { branchSlug, currentBranch } from "./state-git.ts";
import { isoSeconds, stderrWarn, toJqJson, type StateOptions } from "./state-io.ts";
import {
  isTelemetryEvent,
  TELEMETRY_EXTRAS,
  TELEMETRY_VERSION,
  type TelemetryEvent,
  type TelemetryExtras,
} from "./state-schema.ts";

/** Headroom under the 4 KiB atomic-append floor: one `O_APPEND` write stays whole. */
export const TELEMETRY_MAX_LINE_BYTES = 3900;

export type TelemetryResult = { written: true; file: string } | { written: false; reason: string };

function skip(reason: string): TelemetryResult {
  return { written: false, reason };
}

/** The assembled line, or why it cannot be written (a warning in bash). */
function assemble(event: string, extras: unknown, branch: string, now: Date): string | Error {
  if (!isTelemetryEvent(event)) {
    return new Error(`telemetry: unknown event "${event}"; skipping append`);
  }
  const checked = TELEMETRY_EXTRAS[event].safeParse(extras);
  if (!checked.success) {
    return new Error(`telemetry: invalid extras for event "${event}"; skipping append`);
  }
  const line = toJqJson(
    { ...checked.data, v: TELEMETRY_VERSION, t: isoSeconds(now), branch, event },
    false,
  );
  const bytes = Buffer.byteLength(line, "utf8");
  if (bytes > TELEMETRY_MAX_LINE_BYTES) {
    return new Error(
      `telemetry: assembled line for event "${event}" is ${String(bytes)} bytes (>${String(TELEMETRY_MAX_LINE_BYTES)}); skipping append`,
    );
  }
  return line;
}

/** `telemetry_append REPO_ROOT EVENT EXTRAS`. */
export function telemetryAppend<E extends TelemetryEvent>(
  root: string,
  event: E,
  extras: TelemetryExtras[E],
  options: StateOptions = {},
): TelemetryResult {
  if (root === "") return skip("no root");
  const env = options.env ?? process.env;
  const warn = options.warn ?? stderrWarn;
  const host = options.host ?? options.config?.host;
  const scoped = host === undefined ? { env } : { env, host };
  const config = options.config ?? loadConfig({ ...scoped, cwd: root, warn });
  // Default-on: only an explicit false turns telemetry off.
  if (!enabled(config, "telemetry", "enabled")) return skip("disabled");
  const branch = currentBranch(root, env);
  // Unborn or detached HEAD would collapse every caller onto `_default`.
  if (branch === "" || branch === "HEAD") return skip("no branch");
  const line = assemble(event, extras, branch, options.now?.() ?? new Date());
  if (line instanceof Error) {
    warn(line.message);
    return skip(line.message);
  }
  const dir =
    envValue(env, "TELEMETRY_DIR") ??
    projectStateDir("telemetry", { ...scoped, host: config.host, root });
  if (dir === undefined) return skip("no state dir");
  const file = join(dir, `${branchSlug(branch)}.jsonl`);
  try {
    mkdirSync(dir, { recursive: true });
    appendFileSync(file, `${line}\n`);
  } catch (error) {
    return skip(`could not append to ${file}: ${String(error)}`);
  }
  return { written: true, file };
}
