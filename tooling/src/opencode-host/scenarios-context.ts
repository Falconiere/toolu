/**
 * Context, environment, command, UI and event scenarios (#335): what reaches
 * the model from system/prompt/compaction hooks, what `shell.env` and command
 * hooks see, and which notifications arrive on the event bus.
 */
import { z } from "zod";
import { runHost, withServe } from "./host-run.ts";
import {
  allRequestText,
  entries,
  eventTypes,
  messagesText,
  precondition,
  toolRequestCount,
  verdict,
  type Observation,
  type Scenario,
  type ScenarioContext,
} from "./scenario.ts";
import { openSession, PROBE_PLUGIN } from "./session.ts";

const COMMAND_FILE =
  "---\ndescription: Probe command.\n---\nRun PROBE:command.hook with $ARGUMENTS\n";

async function system(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ system: "TOOLU-SYSTEM-CONTEXT" }),
  });
  await runHost(ctx.bin, session, ["PROBE:context.system"]);
  precondition(
    "context.system",
    toolRequestCount(session) > 0,
    "the session never reached the model",
  );
  const reachedModel = messagesText(session, "system").includes("TOOLU-SYSTEM-CONTEXT");
  return verdict(reachedModel, { reachedModel });
}

async function prompt(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ chatPart: "TOOLU-PROMPT-CONTEXT" }),
  });
  await runHost(ctx.bin, session, ["PROBE:context.prompt"]);
  const hookInvoked = entries(session, "chat.message").length > 0;
  precondition(
    "context.prompt",
    hookInvoked && toolRequestCount(session) > 0,
    "chat.message never ran",
  );
  const reachedModel = messagesText(session, "user").includes("TOOLU-PROMPT-CONTEXT");
  return verdict(reachedModel, { hookInvoked, reachedModel });
}

const SessionList = z.array(z.looseObject({ id: z.string() }));

async function summarize(url: string, directory: string): Promise<boolean> {
  const query = `directory=${encodeURIComponent(directory)}`;
  const sessions = SessionList.parse(await (await fetch(`${url}/session?${query}`)).json());
  const id = sessions[0]?.id;
  if (id === undefined) return false;
  const res = await fetch(`${url}/session/${id}/summarize?${query}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ providerID: "probe", modelID: "scripted" }),
  });
  return res.ok;
}

async function compaction(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ compaction: "TOOLU-COMPACTION-CONTEXT" }),
  });
  await runHost(ctx.bin, session, ["PROBE:context.compaction"]);
  const summarized = await withServe(ctx.bin, session, (url) => summarize(url, session.sb.project));
  precondition("context.compaction", summarized, "POST /session/:id/summarize failed");
  const hookInvoked = entries(session, "compacting").length > 0;
  const reachedModel = allRequestText(session).includes("TOOLU-COMPACTION-CONTEXT");
  const compactedEvent = eventTypes(session).includes("session.compacted");
  return verdict(hookInvoked && reachedModel, { hookInvoked, reachedModel, compactedEvent });
}

async function shellEnv(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ shellEnv: "from-shell-env" }),
    config: () => ({ permission: { bash: "allow" } }),
    scripts: {
      "env.shell": [
        {
          tool: "bash",
          args: { command: 'printf %s "$TOOLU_PROBE_ENV" > env.txt', description: "probe" },
        },
      ],
    },
  });
  await runHost(ctx.bin, session, ["PROBE:env.shell"]);
  precondition("env.shell", session.exists("env.txt"), "the bash call never ran");
  const value = session.sb.read("env.txt");
  return verdict(value === "from-shell-env", { value });
}

async function command(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    files: { ".opencode/commands/toolu-probe.md": COMMAND_FILE },
  });
  await runHost(ctx.bin, session, ["--command", "toolu-probe", "argone"]);
  const hook = entries(session, "command.before").find((e) => e.command === "toolu-probe");
  const hookInvoked = hook?.arguments === "argone";
  const eventSeen = eventTypes(session).includes("command.executed");
  const argumentsSubstituted = messagesText(session, "user").includes(
    "Run PROBE:command.hook with argone",
  );
  return verdict(hookInvoked && argumentsSubstituted, {
    hookInvoked,
    eventSeen,
    argumentsSubstituted,
  });
}

async function toast(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ toast: "toolu-probe toast" }),
  });
  await runHost(ctx.bin, session, ["PROBE:ui.toast"]);
  const results = entries(session, "toast");
  precondition("ui.toast", results.length > 0, "session.created never reached the plugin");
  const toastAccepted = results.every((result) => result.ok === true);
  const eventSeen = eventTypes(session).includes("tui.toast.show");
  return verdict(toastAccepted && eventSeen, { toastAccepted, eventSeen });
}

async function bus(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, { localPlugins: [PROBE_PLUGIN] });
  await runHost(ctx.bin, session, ["PROBE:events.bus"]);
  precondition("events.bus", toolRequestCount(session) > 0, "the session never reached the model");
  const types = new Set(eventTypes(session));
  const observed = {
    sessionCreated: types.has("session.created"),
    messageUpdated: types.has("message.updated"),
    sessionIdle: types.has("session.idle"),
  };
  return verdict(Object.values(observed).every(Boolean), observed);
}

export const CONTEXT_SCENARIOS: Scenario[] = [
  {
    id: "context.system",
    axis: "startup",
    kind: "hook",
    mechanism: "experimental.chat.system.transform",
    claim: "experimental.chat.system.transform adds system context that reaches the model",
    run: system,
  },
  {
    id: "context.prompt",
    axis: "prompt",
    kind: "hook",
    mechanism: "chat.message (output.parts)",
    claim: "chat.message can add a text part that reaches the model with the user's prompt",
    run: prompt,
  },
  {
    id: "context.compaction",
    axis: "compaction",
    kind: "hook",
    mechanism: "experimental.session.compacting",
    claim: "experimental.session.compacting adds context that reaches the compaction request",
    run: compaction,
  },
  {
    id: "env.shell",
    axis: "env",
    kind: "hook",
    mechanism: "shell.env",
    claim: "shell.env injects environment variables into bash tool calls",
    run: shellEnv,
  },
  {
    id: "command.hook",
    axis: "surfaces",
    kind: "hook",
    mechanism: "command.execute.before + command.executed",
    claim: "command.execute.before runs for project commands with their arguments",
    run: command,
  },
  {
    id: "ui.toast",
    axis: "ui",
    kind: "event",
    mechanism: "client.tui.showToast → tui.toast.show",
    claim: "A server plugin can publish a TUI toast through client.tui.showToast",
    run: toast,
  },
  {
    id: "events.bus",
    axis: "startup",
    kind: "event",
    mechanism: "event hook",
    claim:
      "The event hook receives bus notifications (session.created, message.updated, session.idle)",
    run: bus,
  },
];
