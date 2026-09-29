import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { encodeDecision } from "../../host/host-encode.ts";
import type { JsonObject, LoadedConfig } from "../config-load.ts";
import {
  GATE_GUARDRAILS,
  GATE_NAMES,
  gateDecision,
  gateMode,
  gatePreset,
  guardrailWarning,
  type GateMode,
} from "../gate-mode.ts";

const GATE_MODE_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/gate-mode.sh",
);

async function bash(script: string, args: string[]): Promise<string> {
  const res = await run(["bash", "-c", `. "$1"; shift; ${script}`, "_", GATE_MODE_SH, ...args], {
    env: { HOME: "/nonexistent-home", TOOLU_CONFIG_DIR: "/nonexistent-config" },
  });
  expect(res.exitCode).toBe(0);
  return res.stdout;
}

function config(data: JsonObject, extra: Partial<LoadedConfig> = {}) {
  const warnings: string[] = [];
  const loaded: LoadedConfig = {
    data,
    invalid: undefined,
    files: { user: "/u", project: undefined },
    host: "claude",
    warn: (message) => warnings.push(message),
    ...extra,
  };
  return { config: loaded, warnings };
}

const REASONS = [
  'Command matches deny rule "node -e".',
  "multi\nline with 'quotes' and \\backslash",
  "unicode ⚠️ — ok",
];

for (const mode of ["block", "ask", "advise"] as const) {
  test.concurrent(`gateDecision(${mode}) encodes like toolu_gate_emit`, async () => {
    for (const reason of REASONS) {
      const expected: unknown = JSON.parse(await bash('toolu_gate_emit "$1" "$2"', [mode, reason]));
      const decision = gateDecision(mode, reason);
      expect(decision).not.toBeNull();
      if (decision === null) return;
      const encoded = encodeDecision("claude", "tool/pre", decision);
      expect(encoded.kind === "command" ? JSON.parse(encoded.stdout) : encoded).toEqual(expected);
    }
  });
}

test.concurrent("off emits nothing in bash and no decision in TS", async () => {
  expect(await bash('toolu_gate_emit off "$1"', ["x"])).toBe("");
  expect(gateDecision("off", "x")).toBeNull();
});

test.concurrent("guardrailWarning is byte-identical to toolu_gate_guardrail_warning", async () => {
  const headline = 'Claude wants to write ".env".';
  const detail = "Secrets live here.\nSecond line.";
  const expected = await bash('toolu_gate_guardrail_warning "$1" "$2"', [headline, detail]);
  expect(guardrailWarning(headline, detail)).toBe(expected);
});

test.concurrent("balanced defaults: qualityGate blocks, guardrails ask, the rest advise", () => {
  const { config: c } = config({});
  expect(gatePreset(c)).toBe("balanced");
  const modes = Object.fromEntries(GATE_NAMES.map((name) => [name, gateMode(c, name)]));
  expect(modes).toEqual({
    pushReview: "advise",
    qualityGate: "block",
    commitGate: "advise",
    bashCommands: "ask",
    planLedger: "advise",
    docsSync: "advise",
    agentTier: "advise",
    protectedFiles: "ask",
    mcpBlocker: "ask",
  });
});

test.concurrent("gates.<name>.mode beats the legacy key, which beats the preset", () => {
  const { config: c, warnings } = config({
    docsSync: { mode: "block" },
    agentTier: { mode: "bogus" },
    gates: { preset: "relaxed", docsSync: { mode: "ask" } },
  });
  expect(gateMode(c, "docsSync")).toBe("ask");
  expect(gateMode(c, "agentTier")).toBe("off");
  expect(gateMode(c, "commitGate")).toBe("off");
  expect(warnings).toEqual([
    "agentTier.mode: 'bogus' is not an allowed value (block ask advise off); using __unset__",
  ]);
});

test.concurrent("ask degrades by class where the host cannot prompt", () => {
  const gates = Object.fromEntries(GATE_NAMES.map((name) => [name, { mode: "ask" }]));
  for (const host of ["codex", "hermes"] as const) {
    const { config: c } = config({ gates }, { host });
    for (const name of GATE_NAMES) {
      const expected: GateMode = GATE_GUARDRAILS.includes(name) ? "block" : "advise";
      expect(gateMode(c, name)).toBe(expected);
    }
  }
  const { config: claude } = config({ gates });
  expect(GATE_NAMES.map((name) => gateMode(claude, name))).toEqual(GATE_NAMES.map(() => "ask"));
  // An explicit host/event overrides the loaded host: Codex can prompt on PermissionRequest.
  expect(gateMode(claude, "pushReview", { host: "codex" })).toBe("advise");
  expect(gateMode(claude, "pushReview", { host: "codex", event: "permission/evaluate" })).toBe(
    "ask",
  );
});

test.concurrent("an invalid config blocks every gate; an unknown gate warns and blocks", () => {
  const { config: c, warnings } = config(
    { gates: { preset: "relaxed" } },
    { invalid: "/p/toolu.config.json: unknown top-level key 'nope'" },
  );
  expect(GATE_NAMES.map((name) => gateMode(c, name))).toEqual(GATE_NAMES.map(() => "block"));
  expect(gateMode(c, "pushReveiw")).toBe("block");
  expect(warnings).toEqual([
    `unknown gate 'pushReveiw' (known: ${GATE_NAMES.join(" ")}); enforcing block`,
  ]);
});
