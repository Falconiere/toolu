/**
 * Permission-composition scenarios (#335): whether a plugin's `permission.ask`
 * hook is consulted, whether a user's config deny holds against a plugin, and
 * whether `tool.execute.before` runs before the native prompt.
 */
import { runHost } from "./host-run.ts";
import {
  entries,
  eventTypes,
  offeredTools,
  precondition,
  toolRequestCount,
  verdict,
  type Observation,
  type Scenario,
  type ScenarioContext,
} from "./scenario.ts";
import { openSession, PROBE_PLUGIN, type ProbeSession } from "./session.ts";

function permissionSession(
  ctx: ScenarioContext,
  id: string,
  bash: string,
  extra: Record<string, string> = {},
): ProbeSession {
  return openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => extra,
    config: () => ({ permission: { bash } }),
    scripts: {
      [id]: [{ tool: "bash", args: { command: "touch permission.txt", description: "probe" } }],
    },
  });
}

async function askHook(ctx: ScenarioContext): Promise<Observation> {
  using session = permissionSession(ctx, "permission.ask-hook", "ask", { permissionAsk: "allow" });
  await runHost(ctx.bin, session, ["PROBE:permission.ask-hook"]);
  const asked = eventTypes(session).includes("permission.asked");
  precondition("permission.ask-hook", asked, "the host never asked for permission");
  const hookInvoked = entries(session, "permission.ask").length > 0;
  return verdict(hookInvoked, { asked, hookInvoked, toolRan: session.exists("permission.txt") });
}

async function configDeny(ctx: ScenarioContext): Promise<Observation> {
  using session = permissionSession(ctx, "permission.config-deny", "deny", {
    permissionAsk: "allow",
  });
  await runHost(ctx.bin, session, ["PROBE:permission.config-deny"]);
  precondition(
    "permission.config-deny",
    toolRequestCount(session) > 0,
    "the session never reached the model",
  );
  const bashOffered = offeredTools(session).includes("bash");
  const sideEffect = session.exists("permission.txt");
  return verdict(!bashOffered && !sideEffect, { bashOffered, sideEffect });
}

async function order(ctx: ScenarioContext): Promise<Observation> {
  using session = permissionSession(ctx, "permission.order", "ask");
  await runHost(ctx.bin, session, ["--auto", "PROBE:permission.order"]);
  const log = session.log();
  const before = log.findIndex((e) => e.kind === "before" && e.tool === "bash");
  const asked = log.findIndex((e) => e.kind === "event" && e.type === "permission.asked");
  precondition(
    "permission.order",
    before >= 0 && asked >= 0,
    "missing before hook or permission.asked event",
  );
  const autoApproved = session.exists("permission.txt");
  return verdict(before < asked, { beforePrecedesAsk: before < asked, autoApproved });
}

export const PERMISSION_SCENARIOS: Scenario[] = [
  {
    id: "permission.ask-hook",
    axis: "permission",
    kind: "hook",
    mechanism: "permission.ask",
    claim: "The plugin permission.ask hook is consulted when the host asks for permission",
    run: askHook,
  },
  {
    id: "permission.config-deny",
    axis: "permission",
    kind: "config",
    mechanism: "opencode.json permission deny",
    claim: "A user's config deny cannot be overridden by a plugin",
    run: configDeny,
  },
  {
    id: "permission.order",
    axis: "permission",
    kind: "hook",
    mechanism: "tool.execute.before vs permission.asked",
    claim: "tool.execute.before runs before the native permission prompt",
    run: order,
  },
];
