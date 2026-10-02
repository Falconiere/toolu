/**
 * Session, prompt and compaction context (#341). Startup lines were captured
 * once per directory. Prompt and compaction text come from the selected
 * plugins' existing bundles. These hooks append; they never refuse a turn.
 */
import { realpathSync } from "node:fs";
import { isAbsolute, relative, resolve, sep } from "node:path";
import type { Hooks } from "@opencode-ai/plugin";
import type { Part, TextPart } from "@opencode-ai/sdk";
import { matcherCovers, pluginHookEntries, type HookEventName } from "../bootstrap/entrypoint.ts";
import {
  parseHookContext,
  type HookContextBody,
  type HookContextEvent,
} from "../bootstrap/hook-context.ts";
import { spawnEntry, type SpawnOutcome, type SpawnRequest } from "../bootstrap/spawn.ts";
import type { LogLevel } from "./context.ts";
import { deletedSessionId, sessionSignals, type SessionSignals } from "./context-sessions.ts";

/** One bundle spawn, long enough for session-start and short enough not to stall a turn. */
const DELIVERY_DEADLINE_MS = 30_000;

export type HookJob = {
  plugin: string;
  name: string;
  bundle: string;
  pluginDir: string;
  event: HookContextEvent;
};

export type ContextPlan = {
  startupLines: readonly string[];
  notices: readonly string[];
  prompt: readonly HookJob[];
  compact: readonly HookJob[];
  bun: string;
  projectRoot: string;
  env: Record<string, string>;
};

export type Spawner = (request: SpawnRequest) => Promise<SpawnOutcome>;

type PluginDir = { name: string; pluginDir: string };

type Log = (level: LogLevel, message: string) => Promise<void>;

type Runner = (
  job: HookJob,
  stdin: string,
  signal: AbortSignal,
) => Promise<HookContextBody | undefined>;

function pathInside(root: string, file: string): boolean {
  const rel = relative(resolve(root), resolve(file));
  return rel !== "" && rel !== ".." && !rel.startsWith(`..${sep}`) && !isAbsolute(rel);
}

/** The bundle must stay inside the scanned plugin directory, including through symlinks. */
function bundleInsidePlugin(pluginDir: string, bundle: string): boolean {
  if (!pathInside(pluginDir, bundle)) return false;
  try {
    return pathInside(realpathSync(pluginDir), realpathSync(bundle));
  } catch (error) {
    return error instanceof Error && "code" in error && error.code === "ENOENT";
  }
}

function appendMissing(target: string[], lines: readonly string[]): void {
  for (const line of lines) {
    if (line !== "" && !target.includes(line)) target.push(line);
  }
}

function userText(parts: readonly Part[]): string {
  const lines: string[] = [];
  for (const part of parts) {
    if (part.type !== "text" || part.synthetic === true || part.text === "") continue;
    lines.push(part.text);
  }
  return lines.join("\n");
}

function jobsFor(
  plugins: readonly PluginDir[],
  event: HookEventName,
  token: string,
): HookJob[] | string {
  const jobs: HookJob[] = [];
  for (const plugin of plugins) {
    const plan = pluginHookEntries(plugin.pluginDir, event, (matcher) =>
      matcherCovers(matcher, token),
    );
    if (!plan.ok) return `${plugin.name}: ${plan.reason}`;
    for (const entry of plan.entries) {
      jobs.push({
        plugin: plugin.name,
        name: entry.name,
        bundle: entry.bundle,
        pluginDir: plugin.pluginDir,
        event,
      });
    }
  }
  return jobs;
}

/** Prompt bundles, then compact SessionStart bundles, then PreCompact bundles. */
export function contextJobs(
  plugins: readonly PluginDir[],
): { ok: true; prompt: HookJob[]; compact: HookJob[] } | { ok: false; reason: string } {
  const prompt = jobsFor(plugins, "UserPromptSubmit", "prompt");
  if (typeof prompt === "string") return { ok: false, reason: prompt };
  const compactStart = jobsFor(plugins, "SessionStart", "compact");
  if (typeof compactStart === "string") return { ok: false, reason: compactStart };
  const preCompact = jobsFor(plugins, "PreCompact", "auto");
  if (typeof preCompact === "string") return { ok: false, reason: preCompact };
  return { ok: true, prompt, compact: [...compactStart, ...preCompact] };
}

function reminderText(body: HookContextBody): string | undefined {
  if (body.kind === "block") return body.reason;
  return body.additionalContext;
}

async function deliverJob(
  plan: ContextPlan,
  log: Log,
  spawn: Spawner,
  job: HookJob,
  stdin: string,
  signal: AbortSignal,
): Promise<HookContextBody | undefined> {
  if (!bundleInsidePlugin(job.pluginDir, job.bundle)) {
    await log("error", `toolu: ${job.plugin}/${job.name}: bundle is outside ${job.pluginDir}`);
    return undefined;
  }
  const env = {
    ...plan.env,
    CLAUDE_PLUGIN_ROOT: job.pluginDir,
    TOOLU_PLUGIN_ROOT: job.pluginDir,
    TOOLU_HOST_OVERRIDE: "opencode",
  };
  let spawned: SpawnOutcome;
  try {
    spawned = await spawn({
      bun: plan.bun,
      bundle: job.bundle,
      cwd: plan.projectRoot,
      env,
      stdin,
      deadlineMs: DELIVERY_DEADLINE_MS,
      signal,
    });
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    await log("error", `toolu: ${job.plugin}/${job.name}: ${reason}`);
    return undefined;
  }
  if (spawned.status === "failed") {
    await log("error", `toolu: ${job.plugin}/${job.name}: ${spawned.reason}`);
    return undefined;
  }
  if (spawned.exitCode !== 0) {
    const said = (spawned.stderr || spawned.stdout).trim();
    await log(
      "error",
      `toolu: ${job.plugin}/${job.name}: exited ${String(spawned.exitCode)} ${said}`,
    );
    return undefined;
  }
  const parsed = parseHookContext(spawned.stdout, job.event);
  if (!parsed.ok) {
    await log("error", `toolu: ${job.plugin}/${job.name}: ${parsed.reason}`);
    return undefined;
  }
  if (parsed.context.kind === "context" && parsed.context.systemMessage !== undefined) {
    await log("info", `toolu: ${job.plugin}/${job.name}: ${parsed.context.systemMessage}`);
  }
  return parsed.context;
}

/** Jobs stay in order. Recursion keeps each spawn after the previous one finishes. */
async function runInOrder(
  jobs: readonly HookJob[],
  index: number,
  signal: AbortSignal,
  step: (job: HookJob) => Promise<void>,
): Promise<void> {
  const job = jobs[index];
  if (job === undefined || signal.aborted) return;
  await step(job);
  await runInOrder(jobs, index + 1, signal, step);
}

async function appendReminders(
  jobs: readonly HookJob[],
  parts: Part[],
  stdin: string,
  signal: AbortSignal,
  nextPart: (text: string) => TextPart,
  run: Runner,
): Promise<Part[]> {
  await runInOrder(jobs, 0, signal, async (job) => {
    const body = await run(job, stdin, signal);
    const reminder = body === undefined ? undefined : reminderText(body);
    if (reminder === undefined || reminder === "") return;
    if (parts.some((part) => part.type === "text" && part.text === reminder)) return;
    parts.push(nextPart(reminder));
  });
  return parts;
}

function compactStdin(plan: ContextPlan, job: HookJob, sessionID: string): string {
  const shared = { session_id: sessionID, cwd: plan.projectRoot };
  return JSON.stringify(
    job.event === "PreCompact"
      ? { hook_event_name: "PreCompact", source: "auto", ...shared }
      : { hook_event_name: "SessionStart", source: "compact", ...shared },
  );
}

async function compactLines(
  plan: ContextPlan,
  sessionID: string,
  signal: AbortSignal,
  run: Runner,
): Promise<string[]> {
  const lines: string[] = [];
  await runInOrder(plan.compact, 0, signal, async (job) => {
    const body = await run(job, compactStdin(plan, job, sessionID), signal);
    const reminder = body === undefined ? undefined : reminderText(body);
    if (reminder !== undefined && reminder !== "") lines.push(reminder);
  });
  lines.push(`toolu sessionID: ${sessionID}`);
  return lines;
}

async function deliverPrompt(
  plan: ContextPlan,
  claimed: Set<string>,
  sessions: SessionSignals,
  input: Parameters<ContextHooks["prompt"]>[0],
  output: Parameters<ContextHooks["prompt"]>[1],
  run: Runner,
  partId: () => string,
): Promise<void> {
  const text = userText(output.parts);
  if (text === "") return;
  const messageID = output.message.id !== "" ? output.message.id : input.messageID;
  const claim =
    messageID === undefined || messageID === "" ? undefined : `${input.sessionID}\0${messageID}`;
  if (claim !== undefined) {
    if (claimed.has(claim)) return;
    claimed.add(claim);
  }
  output.parts = await appendReminders(
    plan.prompt,
    output.parts,
    JSON.stringify({
      hook_event_name: "UserPromptSubmit",
      prompt: text,
      session_id: input.sessionID,
      cwd: plan.projectRoot,
    }),
    sessions.signalFor(input.sessionID),
    (reminder) => ({
      id: partId(),
      sessionID: output.message.sessionID,
      messageID: output.message.id,
      type: "text",
      text: reminder,
      synthetic: true,
    }),
    run,
  );
}

export type ContextHooks = {
  system: NonNullable<Hooks["experimental.chat.system.transform"]>;
  prompt: NonNullable<Hooks["chat.message"]>;
  compacting: NonNullable<Hooks["experimental.session.compacting"]>;
  /** Wider than the SDK `Event` so either published bus shape can be read. */
  event: (input: {
    event: { type: string; properties?: unknown; data?: unknown };
  }) => Promise<void>;
  dispose: () => Promise<void>;
};

/** Hooks for one ready directory. `spawn` is the real child runner except in the abort test. */
export function createContextHooks(
  plan: ContextPlan,
  log: Log,
  spawn: Spawner = spawnEntry,
): ContextHooks {
  const parent = new AbortController();
  const claimed = new Set<string>();
  const sessions = sessionSignals(parent);
  let partSeq = 0;
  const run: Runner = (job, stdin, signal) => deliverJob(plan, log, spawn, job, stdin, signal);
  const system: ContextHooks["system"] = (...[, output]) => {
    appendMissing(output.system, plan.startupLines);
    return Promise.resolve();
  };
  const prompt: ContextHooks["prompt"] = (input, output) =>
    deliverPrompt(plan, claimed, sessions, input, output, run, () => {
      partSeq += 1;
      return `prt_toolu${String(partSeq)}`;
    });
  const compacting: ContextHooks["compacting"] = async (input, output) => {
    appendMissing(
      output.context,
      await compactLines(plan, input.sessionID, sessions.signalFor(input.sessionID), run),
    );
  };
  const event: ContextHooks["event"] = ({ event: bus }) => {
    const sessionID = deletedSessionId(bus);
    if (sessionID !== undefined) sessions.dropSession(sessionID, claimed);
    return Promise.resolve();
  };
  return {
    system,
    prompt,
    compacting,
    event,
    dispose: () => {
      parent.abort();
      sessions.clear();
      claimed.clear();
      return Promise.resolve();
    },
  };
}
