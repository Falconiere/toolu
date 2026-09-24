import type { Host } from "../args/types";
import { CliError, EXIT, UsageError } from "../exit";
import type { SelectHost, SelectHosts } from "../ui/prompts";
import { selectHost as promptSelectHost, selectHosts as promptSelectHosts } from "../ui/prompts";
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

/** True when the host has a working adapter in this CLI. */
export function hasAdapter(host: Host): boolean {
  return ADAPTERS.some((adapter) => adapter.host === host);
}

/** Hosts whose binary resolves on PATH, in probe order. */
export async function availableHosts(env?: NodeJS.ProcessEnv): Promise<readonly Host[]> {
  const found: Host[] = [];
  for (const bin of PROBED) {
    if (await binaryExists(bin, env)) found.push(bin === OPENCODE_BIN ? "opencode" : bin);
  }
  return found;
}

/** Wired hosts on PATH (OpenCode is probed but omitted — no adapter yet). */
export async function selectableHosts(env?: NodeJS.ProcessEnv): Promise<readonly Host[]> {
  return (await availableHosts(env)).filter(hasAdapter);
}

export type HostResolveMode = "single" | "multi";

export interface ResolveHostsOptions {
  readonly interactive: boolean;
  readonly mode: HostResolveMode;
  readonly env?: NodeJS.ProcessEnv;
  readonly selectHosts?: SelectHosts;
  readonly selectHost?: SelectHost;
}

function ambiguousError(available: readonly Host[]): CliError {
  return new CliError(
    EXIT.missingInput,
    `several hosts found (${available.join(", ")}). Choose one with --host.`,
  );
}

/**
 * Resolves one or more hosts to act on.
 *
 * An explicit `--host` always wins. Exactly one wired host on PATH is selected
 * without asking. Ambiguity among wired hosts prompts when interactive; otherwise
 * it refuses and names every probed candidate still on PATH.
 */
export async function resolveHosts(
  requested: Host | undefined,
  options: ResolveHostsOptions,
): Promise<readonly Host[]> {
  if (requested !== undefined) return [requested];

  const available = await availableHosts(options.env);
  if (available.length === 0) {
    throw new CliError(
      EXIT.missingInput,
      `no host found on PATH (probed ${PROBED.join(", ")}). Choose one with --host.`,
    );
  }

  const candidates = available.filter(hasAdapter);
  // Only unwired hosts on PATH (today: opencode). A single one still surfaces so
  // dispatch can explain the missing adapter; more than one is ambiguity.
  if (candidates.length === 0) {
    if (available.length === 1) return available.slice(0, 1);
    throw ambiguousError(available);
  }
  if (candidates.length === 1) {
    const only = candidates[0];
    if (only === undefined) throw ambiguousError(available);
    return [only];
  }

  if (!options.interactive) throw ambiguousError(available);

  if (options.mode === "multi") {
    const pick = options.selectHosts ?? promptSelectHosts;
    const chosen = await pick(candidates);
    if (chosen.length === 0) throw ambiguousError(available);
    return chosen;
  }

  const pickOne = options.selectHost ?? promptSelectHost;
  return [await pickOne(candidates)];
}

/**
 * Resolves a single host. Convenience wrapper over {@link resolveHosts} in
 * single mode — kept for callers that always need exactly one host.
 */
export async function resolveHost(
  requested: Host | undefined,
  env?: NodeJS.ProcessEnv,
  options?: Omit<ResolveHostsOptions, "mode" | "env">,
): Promise<{ host: Host; ambiguous: readonly Host[] }> {
  const hosts = await resolveHosts(requested, {
    interactive: options?.interactive ?? false,
    mode: "single",
    ...(env === undefined ? {} : { env }),
    ...(options?.selectHost === undefined ? {} : { selectHost: options.selectHost }),
    ...(options?.selectHosts === undefined ? {} : { selectHosts: options.selectHosts }),
  });
  const host = hosts[0];
  if (host === undefined) {
    throw new CliError(EXIT.missingInput, "no host selected");
  }
  return { host, ambiguous: [] };
}
