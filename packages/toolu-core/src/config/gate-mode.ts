/**
 * Gate enforcement modes (#253): port of `plugins/toolu/hooks/lib/gate-mode.sh`.
 * A gate owns its check; this decides how firmly a failed check reaches the
 * user. Precedence: `gates.<name>.mode` → the legacy top-level key (docsSync,
 * agentTier) → the `gates.preset` table → `balanced`. Where the host cannot
 * prompt, `ask` degrades: a security guardrail to `block`, a judgement gate to
 * `advise`. An invalid config envelope fails closed: every gate blocks.
 */
import type { Decision } from "../decision/decision.ts";
import { supportsAsk } from "../host/host-encode.ts";
import type { HostEvent } from "../host/host-events.ts";
import type { HostName } from "../host/host-name.ts";
import type { LoadedConfig } from "./config-load.ts";
import { configString } from "./config-read.ts";

export const GATE_MODES = ["block", "ask", "advise", "off"] as const;
export type GateMode = (typeof GATE_MODES)[number];

export const GATE_PRESETS = ["strict", "balanced", "relaxed"] as const;
export type GatePreset = (typeof GATE_PRESETS)[number];
export const DEFAULT_GATE_PRESET: GatePreset = "balanced";

export const GATE_NAMES = [
  "pushReview",
  "qualityGate",
  "commitGate",
  "bashCommands",
  "planLedger",
  "docsSync",
  "agentTier",
  "protectedFiles",
  "mcpBlocker",
] as const;
export type GateName = (typeof GATE_NAMES)[number];

/** Security guardrails: `ask` at every preset but strict, and `ask` degrades to block. */
export const GATE_GUARDRAILS: readonly GateName[] = [
  "protectedFiles",
  "mcpBlocker",
  "bashCommands",
];

/** Gates whose pre-`gates.*` mode key is still honored. */
const LEGACY_PATHS: Readonly<Partial<Record<GateName, string>>> = {
  docsSync: "docsSync.mode",
  agentTier: "agentTier.mode",
};

const UNSET = "__unset__";

function isGateName(name: string): name is GateName {
  return GATE_NAMES.some((gate) => gate === name);
}

/** The preset table: the whole policy. `balanced` is what ships. */
const PRESET_MODE: Readonly<Record<GatePreset, (name: GateName, guardrail: boolean) => GateMode>> =
  {
    strict: () => "block",
    balanced: (name, guardrail) =>
      name === "qualityGate" ? "block" : guardrail ? "ask" : "advise",
    // Relaxed relaxes judgement calls; a guardrail still asks.
    relaxed: (name, guardrail) =>
      guardrail ? "ask" : name === "pushReview" || name === "qualityGate" ? "advise" : "off",
  };

/** `toolu_gate_preset`: the configured preset, or `balanced`. */
export function gatePreset(config: LoadedConfig): GatePreset {
  return configString(config, "gates.preset", DEFAULT_GATE_PRESET, GATE_PRESETS);
}

function readMode(config: LoadedConfig, path: string): GateMode | undefined {
  const mode = configString<GateMode | typeof UNSET>(config, path, UNSET, GATE_MODES);
  return mode === UNSET ? undefined : mode;
}

export type GateModeOptions = { host?: HostName; event?: HostEvent };

/**
 * `toolu_gate_mode NAME`: block | ask | advise | off. An unknown gate name is a
 * caller typo: warn and block, so a misspelled gate keeps enforcing.
 */
export function gateMode(
  config: LoadedConfig,
  name: string,
  options: GateModeOptions = {},
): GateMode {
  if (!isGateName(name)) {
    config.warn(`unknown gate '${name}' (known: ${GATE_NAMES.join(" ")}); enforcing block`);
    return "block";
  }
  if (config.invalid !== undefined) {
    return "block";
  }
  const legacy = LEGACY_PATHS[name];
  const mode =
    readMode(config, `gates.${name}.mode`) ??
    (legacy === undefined ? undefined : readMode(config, legacy)) ??
    PRESET_MODE[gatePreset(config)](name, GATE_GUARDRAILS.includes(name));
  if (mode === "ask" && !supportsAsk(options.host ?? config.host, options.event ?? "tool/pre")) {
    return GATE_GUARDRAILS.includes(name) ? "block" : "advise";
  }
  return mode;
}

/**
 * `toolu_gate_emit MODE REASON` as a core decision: `off` is no output. Encode
 * it with `encodeDecision` for the host's hook JSON.
 */
export function gateDecision(mode: GateMode, reason: string): Decision | null {
  if (mode === "off") {
    return null;
  }
  if (mode === "advise") {
    return { kind: "advisory", message: reason };
  }
  return mode === "ask" ? { kind: "ask", reason } : { kind: "deny", reason };
}

/**
 * `toolu_gate_guardrail_warning HEADLINE DETAIL`: the prompt text for a
 * guardrail's `ask`. Loud on purpose: approving overrides a protection.
 */
export function guardrailWarning(headline: string, detail: string): string {
  return `############################################################
##  ⚠️  SECURITY GUARDRAIL — OVERRIDE REQUESTED  ⚠️        ##
############################################################

${headline}

WHY THIS IS GUARDED
${detail}

Approving covers THIS ONE CALL. Nothing is remembered and the next attempt
asks again. If you did not just ask for this, the answer is no.`;
}
