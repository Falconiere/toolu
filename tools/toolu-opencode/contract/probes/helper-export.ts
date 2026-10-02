/**
 * Host-contract probe (#335): a module that exports a plugin and an ordinary
 * helper function. The live harness observes whether the pinned host invokes
 * the helper as a plugin, and what that does to the session.
 */
import type { Plugin } from "@opencode-ai/plugin";
import { appendFileSync } from "node:fs";

function record(entry: Record<string, unknown>): void {
  const path = process.env.TOOLU_PROBE_LOG;
  if (path !== undefined && path !== "") appendFileSync(path, `${JSON.stringify(entry)}\n`);
}

export function probeHelper(input: unknown): void {
  const keys = typeof input === "object" && input !== null ? Object.keys(input).sort() : [];
  record({ kind: "helper-called", inputKeys: keys });
}

export const TooluHelperExportProbe: Plugin = async () => {
  record({ kind: "plugin-called" });
  return {};
};
