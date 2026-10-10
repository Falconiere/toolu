/**
 * Startup report channel (#342). A host that runs SessionStart entries itself,
 * like the OpenCode bootstrap, cannot tell a registry write that failed (exit
 * 0, one stderr line) or a helper whose source is missing (silent) from a
 * success. When `TOOLU_STARTUP_REPORT` names a file, the startup helpers append
 * one JSON line per contribution there; unset, as on Claude Code and Codex,
 * nothing is written and nothing else changes.
 *
 * Only the writer lives here, with no schema library: every SessionStart bundle
 * carries this module. The reader validates the lines with its own strict schema.
 */
import { appendFileSync } from "node:fs";
import { envValue, type HostEnv } from "../host/host-name.ts";
import type { RegistryEvent } from "../registry/registry-types.ts";

export const STARTUP_REPORT_ENV = "TOOLU_STARTUP_REPORT";

/** One registry module of a plugin's `register` entry, as `registerModules` left it. */
export type RegistryStartupRecord = {
  kind: "registry";
  spec: string;
  name: string;
  event: RegistryEvent;
  source: string;
  target: string;
  status: "written" | "unchanged" | "failed";
  error?: string | undefined;
};

/** A compiled registry rule whose manifest was written by a native SessionStart hook. */
export type NativeRegistryStartupRecord = {
  kind: "native-registry";
  spec: string;
  name: string;
  event: RegistryEvent;
  matcher: string;
  target: string;
};

/** One stable-path helper, as `publishWrapper` left it; `path` is absent when there was no source. */
export type HelperStartupRecord = {
  kind: "helper";
  plugin: string;
  source: string;
  path?: string | undefined;
  status: "published" | "kept-user-file" | "link-failed" | "unwritable" | "source-missing";
};

/** A failure outside any one contribution, e.g. a stale module that could not be pruned. */
export type ErrorStartupRecord = { kind: "error"; origin: string; message: string };

export type StartupRecord =
  | RegistryStartupRecord
  | NativeRegistryStartupRecord
  | HelperStartupRecord
  | ErrorStartupRecord;

/**
 * Append `record` to the report file, if one is named. A record that cannot be
 * written must never read as success, so the failure is one stderr line and
 * `process.exitCode = 1`: the entry still finishes its work, then exits non-zero.
 */
export function reportStartup(record: StartupRecord, env: HostEnv = process.env): void {
  const path = envValue(env, STARTUP_REPORT_ENV);
  if (path === undefined) return;
  try {
    appendFileSync(path, `${JSON.stringify(record)}\n`);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    process.stderr.write(`toolu-startup: cannot write startup report ${path}: ${reason}\n`);
    process.exitCode = 1;
  }
}
