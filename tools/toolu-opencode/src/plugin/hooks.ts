/**
 * Build toolu's `Hooks` for one host instance (#336).
 *
 * The host fails open when plugin init throws (probe `load.init-throw`): it logs
 * the failure and runs every tool unguarded. So nothing here rejects. Any setup
 * failure becomes a `tool.execute.before` that refuses every call, and the reason
 * goes to the host log, which `opencode --print-logs` shows. A ready instance
 * also gives every bash call toolu's helper environment through `shell.env`
 * (#343), and adds the selected plugins' skills, agents and commands to the
 * host config through `config` (#345). A not-ready instance refuses every
 * call, so it adds neither. Either way the verdict is recorded for the status
 * skill and sent as one structured host-log entry (#359).
 */
import type { Hooks } from "@opencode-ai/plugin";
import type { SelectionSource } from "../inventory/selection.ts";
import { createDenyAllToolBefore } from "../adapter/tool-before.ts";
import { applyShellEnv, type ShellEnv } from "../host/runtime-env.ts";
import { applySurfaces, type SurfaceReport } from "../surfaces/apply.ts";
import type { SurfacePlan } from "../surfaces/plan.ts";
import type { HostBinding, LogExtra, LogLevel } from "./context.ts";
import { createContextHooks } from "./context-delivery.ts";
import { prepareEnforcement, type Enforcement } from "./enforcement.ts";
import { claimInstance, releaseInstance } from "./once.ts";
import {
  statusLogExtra,
  statusRecord,
  statusRecordPath,
  writeStatusRecord,
} from "./status-record.ts";

export type PrepareEnforcement = (binding: HostBinding) => Promise<Enforcement>;

/** Startup notes reach the host log, bounded so a noisy cleanup cannot flood it. */
const MAX_DIAGNOSTICS = 20;

async function settle(prepare: PrepareEnforcement, binding: HostBinding): Promise<Enforcement> {
  try {
    return await prepare(binding);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return { status: "not-ready", reason: `setup failed: ${reason}` };
  }
}

/**
 * The diagnostic is advisory: enforcement is already decided when it is sent, so a
 * log that throws or rejects is dropped rather than allowed to abort init after the
 * directory was claimed (which would leave the claim held with no `dispose`).
 */
async function report(
  binding: HostBinding,
  level: LogLevel,
  message: string,
  extra?: LogExtra,
): Promise<void> {
  try {
    await binding.log(level, message, extra);
  } catch {
    return;
  }
}

/** `shell.env`: add toolu's variables to the env the host builds for one bash call. */
function shellEnvHook(shell: ShellEnv): NonNullable<Hooks["shell.env"]> {
  return (...[, output]) => {
    applyShellEnv(shell, output.env);
    return Promise.resolve();
  };
}

/** Notes as one bounded log line, so a slow host log costs one call, not one per note. */
function notesLine(label: string, notes: readonly string[]): string | undefined {
  if (notes.length === 0) return undefined;
  const shown = notes.slice(0, MAX_DIAGNOSTICS).join("; ");
  const more = notes.length - MAX_DIAGNOSTICS;
  return `toolu: ${label}: ${shown}${more > 0 ? ` (${more} more)` : ""}`;
}

export function startupNotes(diagnostics: readonly string[]): string | undefined {
  return notesLine("startup notes", diagnostics);
}

const SOURCE_LABEL: Record<SelectionSource, string> = {
  project: "project selection",
  global: "global selection",
  default: "all installed plugins",
};

function idList(list: readonly string[]): string {
  return list.length > 0 ? list.join(", ") : "none";
}

/** The owned contribution, by ID, and every overlap with the user's own definitions. */
function surfaceLines(applied: SurfaceReport, source: SelectionSource, ms: number): string[] {
  const { skills, agents, commands } = applied;
  const owned = `skills ${idList(skills)}; agents ${idList(agents)}; commands ${idList(commands)}`;
  const notes = notesLine("surface notes", [
    ...applied.kept.map(({ id, location }) => `skill ${id} kept from ${location}`),
    ...applied.merged.map(({ kind, id }) => `${kind} ${id} merged under your config`),
    ...applied.notes,
  ]);
  const head = `toolu: surfaces (${SOURCE_LABEL[source]}, ${Math.round(ms)} ms): ${owned}`;
  return notes === undefined ? [head] : [head, notes];
}

/** `config`: add the planned surfaces to the host's merged config; never throws. */
function surfacesHook(
  binding: HostBinding,
  plan: SurfacePlan,
  source: SelectionSource,
): NonNullable<Hooks["config"]> {
  return async (config) => {
    const started = performance.now();
    let lines: string[];
    try {
      const scope = { directory: binding.directory, worktree: binding.worktree, env: binding.env };
      lines = surfaceLines(applySurfaces(config, plan, scope), source, performance.now() - started);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      await report(binding, "error", `toolu: surfaces not applied: ${reason}`);
      return;
    }
    await Promise.all(lines.map((line) => report(binding, "info", line)));
  };
}

/** Record the verdict for the status skill, then send it as one structured entry; never throws. */
async function reportStatus(binding: HostBinding, enforcement: Enforcement): Promise<void> {
  const record = statusRecord(enforcement, binding.projectRoot, new Date());
  let path: string;
  try {
    path = statusRecordPath(binding);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    await report(binding, "error", `toolu: status record not written: ${reason}`);
    return;
  }
  const failure = writeStatusRecord(path, record);
  if (failure !== undefined) {
    await report(binding, "error", `toolu: status record not written: ${failure}`);
  }
  const level = record.status === "ready" ? "info" : "error";
  await report(binding, level, "toolu: status", statusLogExtra(record, path));
}

/** Never rejects: a failure to prepare enforcement yields a hook that denies every tool call. */
export async function createTooluHooks(
  binding: HostBinding,
  prepare: PrepareEnforcement = prepareEnforcement,
): Promise<Hooks> {
  if (!claimInstance(binding.directory)) {
    await report(binding, "info", `toolu: duplicate load skipped for ${binding.directory}`);
    // No hooks: the admitted instance enforces. Its dispose, not this one, releases the claim.
    return { dispose: () => Promise.resolve() };
  }
  const enforcement = await settle(prepare, binding);
  const dispose = (): Promise<void> => {
    if (enforcement.status === "ready") enforcement.clearAdvice();
    releaseInstance(binding.directory);
    return Promise.resolve();
  };
  if (enforcement.status === "ready") {
    const { plugins, artifacts, diagnostics } = enforcement;
    await report(
      binding,
      "info",
      `toolu: ready (${plugins.length} plugins, ${artifacts.length} startup artifacts)`,
    );
    const notes = startupNotes(diagnostics);
    if (notes !== undefined) await report(binding, "info", notes);
    await Promise.all(enforcement.context.notices.map((notice) => report(binding, "info", notice)));
    await reportStatus(binding, enforcement);
    const context = createContextHooks(enforcement.context, (level, message) =>
      report(binding, level, message),
    );
    return {
      "tool.execute.before": enforcement.before,
      "shell.env": shellEnvHook(enforcement.shellEnv),
      "tool.execute.after": enforcement.after,
      config: surfacesHook(binding, enforcement.surfaces, enforcement.selectionSource),
      "experimental.chat.system.transform": context.system,
      "chat.message": context.prompt,
      "experimental.session.compacting": context.compacting,
      event: context.event,
      dispose: async () => {
        await context.dispose();
        await dispose();
      },
    };
  }
  const message = `toolu: not ready: ${enforcement.reason}`;
  await report(binding, "error", `${message}; every tool call is denied`);
  await reportStatus(binding, enforcement);
  return { "tool.execute.before": createDenyAllToolBefore(message), dispose };
}
