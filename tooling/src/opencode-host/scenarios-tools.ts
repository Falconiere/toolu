/**
 * Enforcement scenarios (#335): whether a `tool.execute.before` throw blocks
 * each tool class before its side effect, whether a pre-tool hook can add
 * advisory context, and how plugin hooks compose with native permissions.
 */
import { join } from "node:path";
import { existsSync } from "node:fs";
import { runHost, toolStates } from "./host-run.ts";
import type { ScriptStep } from "./provider.ts";
import {
  entries,
  hookedTools,
  messagesText,
  offeredTools,
  precondition,
  verdict,
  type Observation,
  type Scenario,
  type ScenarioContext,
} from "./scenario.ts";
import {
  DENY_MARKER,
  DENY_MESSAGE,
  openSession,
  PROBE_PLUGIN,
  type ProbeSession,
  type SessionOptions,
} from "./session.ts";

const MCP_SERVER = join(import.meta.dir, "mcp-server.ts");

type DenyCase = {
  tool: string;
  step: ScriptStep[];
  /** Whether the denied call left any trace: a file, changed bytes, or output the model saw. */
  sideEffect: (session: ProbeSession) => boolean;
  options?: SessionOptions;
};

function created(...rels: string[]): (session: ProbeSession) => boolean {
  return (session) => rels.some((rel) => session.exists(rel));
}

async function denyCase(ctx: ScenarioContext, id: string, c: DenyCase): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ denyMarker: DENY_MARKER }),
    config: () => ({ permission: { bash: "allow", edit: "allow" } }),
    ...c.options,
    scripts: { [id]: c.step },
  });
  const run = await runHost(ctx.bin, session, [`PROBE:${id}`]);
  const beforeInvoked = hookedTools(session, "before").includes(c.tool);
  precondition(id, beforeInvoked, `tool.execute.before never saw ${c.tool}`);
  const sideEffect = c.sideEffect(session);
  const errorReachedModel = messagesText(session, "tool").includes(DENY_MESSAGE);
  const afterInvoked = hookedTools(session, "after").includes(c.tool);
  const blocked = toolStates(run).some((t) => t.tool === c.tool && t.status === "error");
  return verdict(!sideEffect && errorReachedModel && blocked, {
    beforeInvoked,
    sideEffect,
    errorReachedModel,
    afterInvoked,
  });
}

const PATCH = [
  "*** Begin Patch",
  "*** Add File: ok-patch.txt",
  "+fine",
  `*** Add File: ${DENY_MARKER}-patch.txt`,
  "+secret",
  "*** End Patch",
].join("\n");

function mcpConfig(root: string): Record<string, unknown> {
  const environment = { MCP_MARKER: join(root, "mcp-marker.txt") };
  return {
    mcp: { probe: { type: "local", command: [process.execPath, MCP_SERVER], environment } },
  };
}

async function denyMcp(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ denyMarker: DENY_MARKER }),
    config: mcpConfig,
    scripts: { "deny.mcp": [{ tool: "probe_touch", args: { name: DENY_MARKER } }] },
  });
  await runHost(ctx.bin, session, ["PROBE:deny.mcp"]);
  const toolOffered = offeredTools(session).includes("probe_touch");
  precondition(
    "deny.mcp",
    toolOffered && hookedTools(session, "before").includes("probe_touch"),
    "the MCP tool never reached tool.execute.before",
  );
  const sideEffect = existsSync(session.outside("mcp-marker.txt"));
  const errorReachedModel = messagesText(session, "tool").includes(DENY_MESSAGE);
  return verdict(!sideEffect && errorReachedModel, {
    toolName: "probe_touch",
    sideEffect,
    errorReachedModel,
  });
}

async function denyTaskChild(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ denyMarker: DENY_MARKER }),
    config: () => ({ permission: { bash: "allow", task: "allow" } }),
    scripts: {
      "deny.task-child": [
        {
          tool: "task",
          args: {
            description: "child probe",
            prompt: "PROBE:task-child-run",
            subagent_type: "general",
          },
        },
      ],
      "task-child-run": [
        {
          tool: "bash",
          args: {
            command: `touch child-ok.txt && touch ${DENY_MARKER}-child.txt`,
            description: "child",
          },
        },
      ],
    },
  });
  await runHost(ctx.bin, session, ["PROBE:deny.task-child"]);
  const befores = entries(session, "before");
  const task = befores.find((e) => e.tool === "task");
  const child = befores.find((e) => e.tool === "bash");
  precondition(
    "deny.task-child",
    task !== undefined,
    "tool.execute.before never saw the task tool",
  );
  const childHookInvoked = child !== undefined && child.sessionID !== task?.sessionID;
  const sideEffect = session.exists("child-ok.txt") || session.exists(`${DENY_MARKER}-child.txt`);
  return verdict(childHookInvoked && !sideEffect, {
    taskHookInvoked: true,
    childHookInvoked,
    sideEffect,
  });
}

async function preAdvisory(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => ({ preAdvisory: "TOOLU-PRE-ADVISORY" }),
    config: () => ({ permission: { bash: "allow" } }),
    scripts: {
      "pre.advisory": [{ tool: "bash", args: { command: "echo probe", description: "probe" } }],
    },
  });
  await runHost(ctx.bin, session, ["PROBE:pre.advisory"]);
  const toolRan = hookedTools(session, "after").includes("bash");
  precondition("pre.advisory", toolRan, "the bash call never completed");
  const advisoryReachedModel =
    messagesText(session, "tool").includes("TOOLU-PRE-ADVISORY") ||
    messagesText(session, "system").includes("TOOLU-PRE-ADVISORY");
  return verdict(advisoryReachedModel, { toolRan, advisoryReachedModel });
}

function deny(
  id: Scenario["id"],
  tool: string,
  c: Omit<DenyCase, "tool">,
  mechanism: string,
  claim: string,
): Scenario {
  return {
    id,
    axis: "tools",
    kind: "hook",
    mechanism,
    claim,
    run: (ctx) => denyCase(ctx, id, { tool, ...c }),
  };
}

export const TOOL_SCENARIOS: Scenario[] = [
  deny(
    "deny.bash",
    "bash",
    {
      step: [
        { tool: "bash", args: { command: `touch ${DENY_MARKER}-bash.txt`, description: "probe" } },
      ],
      sideEffect: created(`${DENY_MARKER}-bash.txt`),
    },
    "tool.execute.before (throw)",
    "A tool.execute.before throw blocks a bash call before it runs and the model sees the reason",
  ),
  deny(
    "deny.write",
    "write",
    {
      step: [{ tool: "write", args: { filePath: `${DENY_MARKER}-write.txt`, content: "secret" } }],
      sideEffect: created(`${DENY_MARKER}-write.txt`),
    },
    "tool.execute.before (throw)",
    "A tool.execute.before throw blocks a write before the file changes",
  ),
  deny(
    "deny.edit",
    "edit",
    {
      step: [
        {
          tool: "edit",
          args: {
            filePath: "edit-target.txt",
            oldString: "before",
            newString: `${DENY_MARKER}-after`,
          },
        },
      ],
      sideEffect: (session) => session.sb.read("edit-target.txt") !== "before\n",
      options: { files: { "edit-target.txt": "before\n" } },
    },
    "tool.execute.before (throw)",
    "A tool.execute.before throw blocks an edit before the file's bytes change",
  ),
  deny(
    "deny.grep",
    "grep",
    {
      step: [{ tool: "grep", args: { pattern: DENY_MARKER, path: "." } }],
      sideEffect: (session) => messagesText(session, "tool").includes("grep-needle.txt"),
      options: { files: { "grep-needle.txt": `needle-${DENY_MARKER}\n` } },
    },
    "tool.execute.before (throw)",
    "A tool.execute.before throw blocks a grep before its results reach the model",
  ),
  deny(
    "deny.apply-patch",
    "apply_patch",
    {
      step: [{ tool: "apply_patch", args: { patchText: PATCH } }],
      sideEffect: created("ok-patch.txt", `${DENY_MARKER}-patch.txt`),
      options: { model: "gpt-5-probe" },
    },
    "tool.execute.before (throw)",
    "A tool.execute.before throw blocks a whole multi-file apply_patch (gpt-* models) before any file changes",
  ),
  {
    id: "deny.mcp",
    axis: "mcp",
    kind: "hook",
    mechanism: "tool.execute.before on <server>_<tool>",
    claim: "A tool.execute.before throw blocks an MCP tool call before the server receives it",
    run: denyMcp,
  },
  {
    id: "deny.task-child",
    axis: "task",
    kind: "hook",
    mechanism: "tool.execute.before on task + child session",
    claim:
      "tool.execute.before sees the task tool and child-session tools, and its throw blocks them",
    run: denyTaskChild,
  },
  {
    id: "pre.advisory",
    axis: "tools",
    kind: "hook",
    mechanism: "tool.execute.before output fields",
    claim: "tool.execute.before can add non-blocking advisory context that reaches the model",
    run: preAdvisory,
  },
];
