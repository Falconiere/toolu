/**
 * Post-tool scenarios (#335): whether `tool.execute.after` feedback reaches
 * the model, and which tool outcomes (non-zero bash exit, thrown error) it sees.
 */
import type { ScriptStep } from "./provider.ts";
import { runHost } from "./host-run.ts";
import {
  entries,
  finalMessages,
  hookedTools,
  messagesText,
  precondition,
  verdict,
  type Observation,
  type Scenario,
  type ScenarioContext,
} from "./scenario.ts";
import { openSession, PROBE_PLUGIN, type ProbeSession } from "./session.ts";

function postSession(
  ctx: ScenarioContext,
  id: string,
  steps: ScriptStep[],
  cfg: Record<string, string> = {},
): ProbeSession {
  return openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: () => cfg,
    config: () => ({ permission: { bash: "allow", edit: "allow" } }),
    scripts: { [id]: steps },
  });
}

async function feedback(ctx: ScenarioContext): Promise<Observation> {
  using session = postSession(
    ctx,
    "post.feedback",
    [
      { tool: "write", args: { filePath: "a.txt", content: "one\n" } },
      { tool: "edit", args: { filePath: "a.txt", oldString: "one", newString: "two" } },
    ],
    { afterAppend: "TOOLU-POST-FEEDBACK" },
  );
  await runHost(ctx.bin, session, ["PROBE:post.feedback"]);
  precondition(
    "post.feedback",
    hookedTools(session, "after").length >= 2,
    "write/edit never completed",
  );
  const feedbackCount = finalMessages(session, "tool").filter((m) =>
    m.includes("TOOLU-POST-FEEDBACK"),
  ).length;
  return verdict(feedbackCount === 2, {
    feedbackCount,
    fileContent: session.sb.read("a.txt").trim(),
  });
}

async function bashExit(ctx: ScenarioContext): Promise<Observation> {
  using session = postSession(ctx, "post.bash-exit", [
    { tool: "bash", args: { command: "echo out; exit 3", description: "probe" } },
  ]);
  await runHost(ctx.bin, session, ["PROBE:post.bash-exit"]);
  precondition(
    "post.bash-exit",
    hookedTools(session, "before").includes("bash"),
    "bash never reached tool.execute.before",
  );
  const after = entries(session, "after").find((e) => e.tool === "bash");
  const exit = after?.exit ?? -1;
  return verdict(after !== undefined && exit === 3, { afterInvoked: after !== undefined, exit });
}

async function toolError(ctx: ScenarioContext): Promise<Observation> {
  using session = postSession(ctx, "post.tool-error", [
    { tool: "read", args: { filePath: "missing.txt" } },
  ]);
  await runHost(ctx.bin, session, ["PROBE:post.tool-error"]);
  const beforeInvoked = hookedTools(session, "before").includes("read");
  const errorReachedModel = messagesText(session, "tool").includes("File not found");
  precondition("post.tool-error", beforeInvoked && errorReachedModel, "the failing read never ran");
  const afterInvoked = hookedTools(session, "after").includes("read");
  return verdict(afterInvoked, { beforeInvoked, afterInvoked, errorReachedModel });
}

export const POST_SCENARIOS: Scenario[] = [
  {
    id: "post.feedback",
    axis: "postTool",
    kind: "hook",
    mechanism: "tool.execute.after (output append)",
    claim: "tool.execute.after can append feedback that reaches the model",
    run: feedback,
  },
  {
    id: "post.bash-exit",
    axis: "postTool",
    kind: "hook",
    mechanism: "tool.execute.after metadata.exit",
    claim:
      "tool.execute.after runs for a bash command that exits non-zero and reports its exit code",
    run: bashExit,
  },
  {
    id: "post.tool-error",
    axis: "postTool",
    kind: "hook",
    mechanism: "tool.execute.after on a thrown tool error",
    claim: "tool.execute.after runs when a tool call fails",
    run: toolError,
  },
];
