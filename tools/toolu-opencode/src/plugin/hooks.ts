/**
 * Build toolu's `Hooks` for one host instance (#336).
 *
 * The host fails open when plugin init throws (probe `load.init-throw`): it logs
 * the failure and runs every tool unguarded. So nothing here rejects. Any setup
 * failure becomes a `tool.execute.before` that refuses every call, and the reason
 * goes to the host log, which `opencode --print-logs` shows.
 */
import type { Hooks } from "@opencode-ai/plugin";
import { createDenyAllToolBefore } from "../adapter/tool-before.ts";
import type { HostBinding, LogLevel } from "./context.ts";
import { prepareEnforcement, type Enforcement } from "./enforcement.ts";
import { claimInstance, releaseInstance } from "./once.ts";

export type PrepareEnforcement = (binding: HostBinding) => Promise<Enforcement>;

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
    await report(
      binding,
      "info",
      `toolu: ready (${enforcement.artifacts.length} bootstrap artifacts)`,
    );
    return { "tool.execute.before": enforcement.before, dispose };
  }
  const message = `toolu: not ready: ${enforcement.reason}`;
  await report(binding, "error", `${message}; every tool call is denied`);
  return { "tool.execute.before": createDenyAllToolBefore(message), dispose };
}
