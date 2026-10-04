#!/usr/bin/env bun
/** Parse the committed generated surface on the pinned, isolated OpenCode host. */
import { hostCacheDir, resolveHostBinary } from "./opencode-host/install.ts";
import { contractPaths } from "./opencode-host/results.ts";
import { PinSchema, readJson } from "./opencode-host/schema.ts";
import { probeGeneratedSurface } from "./opencode-host/surface-probe.ts";

const pin = readJson(contractPaths().pin, PinSchema);
const host = await resolveHostBinary(pin);
const counts = await probeGeneratedSurface(host.bin, hostCacheDir(pin));
process.stdout.write(
  `opencode-surface-probe: ${host.version}, ${counts.plugins} plugins, ${counts.skills} skills, ${counts.agents} agents, ${counts.commands} commands loaded\n`,
);
