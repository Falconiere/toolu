/**
 * Host-contract probe (#335): a default `PluginModule` beside named exports.
 * The live harness observes that the pinned host invokes only `server`.
 */
import type { PluginModule } from "@opencode-ai/plugin";
import { appendFileSync } from "node:fs";

function record(entry: Record<string, unknown>): void {
  const path = process.env.TOOLU_PROBE_LOG;
  if (path !== undefined && path !== "") appendFileSync(path, `${JSON.stringify(entry)}\n`);
}

export function namedHelper(): void {
  record({ kind: "named-helper-called" });
}

export const namedValue = "ignored when a default PluginModule is exported";

const probeModule: PluginModule = {
  id: "toolu-contract-probe",
  server: async () => {
    record({ kind: "module-server-called" });
    return {};
  },
};

export default probeModule;
