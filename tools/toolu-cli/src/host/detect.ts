import type { Host } from "../args/types";
import { CliError, EXIT, UsageError } from "../exit";
import { claudeAdapter } from "./claude";
import { codexAdapter } from "./codex";
import { binaryExists } from "./run";
import type { HostAdapter } from "./types";

const ADAPTERS: readonly HostAdapter[] = [claudeAdapter, codexAdapter];
const OPENCODE_BIN = "opencode";
const PROBED = ["claude", "codex", OPENCODE_BIN] as const;

/**
 * The adapter for an explicitly requested host.
 *
 * OpenCode has its own plugin CLI (`opencode plugin add|list|remove`), but it
 * installs npm packages rather than marketplace entries, so the 13 bash plugins
 * are not addressable through it. Its adapter installs the `@toolu/opencode`
 * bridge instead, and does not exist yet.
 */
export function adapterFor(host: Host): HostAdapter {
  const adapter = ADAPTERS.find((candidate) => candidate.host === host);
  if (adapter === undefined) {
    throw new UsageError(`${host} has no adapter yet; see docs/opencode.md`);
  }
  return adapter;
}

/** Hosts whose binary resolves on PATH, in probe order. */
export async function availableHosts(env?: NodeJS.ProcessEnv): Promise<readonly Host[]> {
  const found: Host[] = [];
  for (const bin of PROBED) {
    if (await binaryExists(bin, env)) found.push(bin === OPENCODE_BIN ? "opencode" : bin);
  }
  return found;
}

/**
 * Resolves the host to act on. An explicit choice always wins, and exactly one
 * available host is selected without asking. Ambiguity always refuses and names
 * the candidates, so a host is never chosen on the user's behalf.
 */
export async function resolveHost(
  requested: Host | undefined,
  env?: NodeJS.ProcessEnv,
): Promise<{ host: Host; ambiguous: readonly Host[] }> {
  if (requested !== undefined) return { host: requested, ambiguous: [] };
  const available = await availableHosts(env);
  if (available.length === 0) {
    throw new CliError(
      EXIT.missingInput,
      `no host found on PATH (probed ${PROBED.join(", ")}). Choose one with --host.`,
    );
  }
  const only = available[0];
  if (available.length === 1 && only !== undefined) return { host: only, ambiguous: [] };
  // Ambiguity is never resolved silently. Interactive selection arrives with the
  // prompt layer; until then both paths refuse and name the choice.
  throw new CliError(
    EXIT.missingInput,
    `several hosts found (${available.join(", ")}). Choose one with --host.`,
  );
}
