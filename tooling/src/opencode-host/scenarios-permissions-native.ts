/** Native deny and guardrail fallback on the pinned host. */
import { join } from "node:path";
import { runHost, toolStates } from "./host-run.ts";
import {
  denied,
  ENV_BYTES,
  session,
  SMOKE_RUN_TIMEOUT_MS,
  type PretoolScenario,
} from "./pretool-shared.ts";
import { entries, offeredTools, type ScenarioContext } from "./scenario.ts";

async function nativeDeny(ctx: ScenarioContext) {
  using s = session(ctx, {
    config: () => ({ permission: { bash: { "*": "allow", "touch native-deny-marker": "deny" } } }),
    scripts: {
      "permissions.native-deny": [
        { tool: "bash", args: { command: "touch native-deny-marker", description: "native deny" } },
      ],
    },
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:permissions.native-deny"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const observed = {
    bashOffered: offeredTools(s).includes("bash"),
    tooluBeforeRan: entries(s, "before").some((entry) => entry.tool === "bash"),
    markerAbsent: !s.exists("native-deny-marker"),
    nativeDenied: toolStates(run).some(
      (state) => state.tool === "bash" && state.status === "error",
    ),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function guardrailAsk(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { protectedFiles: { mode: "ask" } },
      }),
    },
    scripts: (project) => ({
      "permissions.guardrail": [
        { tool: "write", args: { filePath: join(project, ".env"), content: "PWNED\n" } },
      ],
    }),
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:permissions.guardrail"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const observed = {
    gateDenied: denied(toolStates(run), "write", /protected/i),
    envUnchanged: s.sb.read(".env") === ENV_BYTES,
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

export const NATIVE_SCENARIOS: PretoolScenario[] = [
  { id: "permissions.native-deny", run: nativeDeny },
  { id: "permissions.guardrail", run: guardrailAsk },
];
