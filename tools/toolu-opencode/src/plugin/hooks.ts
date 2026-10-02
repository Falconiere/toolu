/**
 * Build toolu's `Hooks` for one host instance (#336).
 *
 * The host fails open when plugin init throws (probe `load.init-throw`): it logs
 * the failure and runs every tool unguarded. So nothing here rejects. Any setup
 * failure becomes a `tool.execute.before` that refuses every call, and the reason
 * goes to the host log, which `opencode --print-logs` shows. A ready instance
 * also gives every bash call toolu's helper environment through `shell.env`
 * (#343); a not-ready one refuses bash, so it adds none.
 */
import type { Hooks } from "@opencode-ai/plugin";
import { createDenyAllToolBefore } from "../adapter/tool-before.ts";
import type { HostBinding, LogLevel } from "./context.ts";
import { prepareEnforcement, type Enforcement } from "./enforcement.ts";
import { claimInstance, releaseInstance } from "./once.ts";

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
async function report(binding: HostBinding, level: LogLevel, message: string): Promise<void> {
  try {
    await binding.log(level, message);
  } catch {
    return;
  }
}

/** `shell.env`: add toolu's variables to the env the host builds for one bash call. */
function shellEnvHook(
  additions: Readonly<Record<string, string>>,
): NonNullable<Hooks["shell.env"]> {
  return (...[, output]) => {
    Object.assign(output.env, additions);
    return Promise.resolve();
  };
}

/** Startup notes as one log line, so a slow host log costs one bounded call, not one per note. */
export function startupNotes(diagnostics: readonly string[]): string | undefined {
  if (diagnostics.length === 0) return undefined;
  const shown = diagnostics.slice(0, MAX_DIAGNOSTICS).join("; ");
  const more = diagnostics.length - MAX_DIAGNOSTICS;
  return `toolu: startup notes: ${shown}${more > 0 ? ` (${more} more)` : ""}`;
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
    return {
      "tool.execute.before": enforcement.before,
      "shell.env": shellEnvHook(enforcement.shellEnv),
      dispose,
    };
  }
  const message = `toolu: not ready: ${enforcement.reason}`;
  await report(binding, "error", `${message}; every tool call is denied`);
  return { "tool.execute.before": createDenyAllToolBefore(message), dispose };
}
