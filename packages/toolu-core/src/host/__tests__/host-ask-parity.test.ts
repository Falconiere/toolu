/**
 * `ask` degradation parity with `plugins/toolu/hooks/lib/gate-mode.sh`: every
 * gate configured `ask` in a real project config resolves, per host, to the same
 * mode in bash `toolu_gate_mode` and in `degradeAsk` + `supportsAsk`.
 */
import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import type { Decision } from "../../decision/decision.ts";
import { detectHost } from "../host-detect.ts";
import { degradeAsk, supportsAsk } from "../host-encode.ts";

const GATE_MODE_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/gate-mode.sh",
);
const HOST_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/host.sh");
const root = realpathSync(mkdtempSync(join(tmpdir(), "host-ask-parity-")));
afterAll(() => rmSync(root, { recursive: true, force: true }));

function bash(script: string, lib: string, env: Record<string, string> = {}): string {
  const res = spawnSync("bash", ["-c", `. "$1"; ${script}`, "_", lib], {
    env: { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: join(root, "home"), ...env },
    encoding: "utf8",
  });
  expect(res.status).toBe(0);
  return res.stdout;
}

const GATES = bash('printf "%s" "$TOOLU_GATE_NAMES"', GATE_MODE_SH).split(" ");
const GUARDRAILS = new Set(bash('printf "%s" "$TOOLU_GATE_GUARDRAILS"', GATE_MODE_SH).split(" "));

function mode(decision: Decision): string {
  return decision.kind === "deny"
    ? "block"
    : decision.kind === "advisory"
      ? "advise"
      : decision.kind;
}

function projectWithAskEverywhere(host: "claude" | "codex"): string {
  const project = join(root, host);
  const gates = Object.fromEntries(GATES.map((name) => [name, { mode: "ask" }]));
  mkdirSync(join(project, `.${host}`), { recursive: true });
  writeFileSync(join(project, `.${host}`, "toolu.config.json"), JSON.stringify({ gates }));
  return project;
}

describe("ask degradation matches gate-mode.sh", () => {
  test("the bash gate lists are non-empty and guardrails are gates", () => {
    expect(GATES.length).toBeGreaterThan(3);
    for (const guardrail of GUARDRAILS) expect(GATES).toContain(guardrail);
  });

  for (const host of ["claude", "codex"] as const) {
    test(`${host}: every gate configured ask resolves to the bash mode`, () => {
      const env = {
        TOOLU_HOST_OVERRIDE: host,
        TOOLU_PROJECT_DIR: projectWithAskEverywhere(host),
        TOOLU_CONFIG_DIR: join(root, "user-config"),
      };
      const script = GATES.map((name) => `printf '%s\\n' "$(toolu_gate_mode ${name})"`).join("; ");
      const bashModes = bash(script, GATE_MODE_SH, env).trim().split("\n");
      const tsHost = detectHost({ env });
      const tsModes = GATES.map((name) => {
        const ask: Decision = { kind: "ask", reason: `${name} asks` };
        const gateClass = GUARDRAILS.has(name) ? "guardrail" : "judgement";
        return mode(degradeAsk(tsHost, "tool/pre", ask, gateClass));
      });
      expect(bashModes).toEqual(tsModes);
      expect(new Set(bashModes)).toEqual(new Set(host === "codex" ? ["block", "advise"] : ["ask"]));
    });
  }

  test("supportsAsk agrees with bash toolu_supports_ask on Claude and Codex", () => {
    for (const host of ["claude", "codex"] as const) {
      const out = bash("toolu_supports_ask && echo yes || echo no", HOST_SH, {
        TOOLU_HOST_OVERRIDE: host,
      });
      expect(out.trim()).toBe(supportsAsk(host) ? "yes" : "no");
    }
  });
});
