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
import type { HostBinding } from "./context.ts";
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

/** Never rejects: a failure to prepare enforcement yields a hook that denies every tool call. */
export async function createTooluHooks(
  binding: HostBinding,
  prepare: PrepareEnforcement = prepareEnforcement,
): Promise<Hooks> {
  if (!claimInstance(binding.directory)) {
    await binding.log("info", `toolu: duplicate load skipped for ${binding.directory}`);
    return {};
  }
  const enforcement = await settle(prepare, binding);
  const dispose = (): Promise<void> => {
    releaseInstance(binding.directory);
    return Promise.resolve();
  };
  if (enforcement.status === "ready") {
    await binding.log("info", `toolu: ready (${enforcement.artifacts.length} bootstrap artifacts)`);
    return { "tool.execute.before": enforcement.before, dispose };
  }
  const message = `toolu: not ready: ${enforcement.reason}`;
  await binding.log("error", `${message}; every tool call is denied`);
  return { "tool.execute.before": createDenyAllToolBefore(message), dispose };
}
