/** Assemble registry via argv-only bash runner (#211). */
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { createBunBashRunner, type BashRunner } from "@toolu/core/runner";
import { opencodeDataRoot } from "../host/roots.ts";
import type { PluginManifest } from "../inventory/types.ts";
import { notReady, ready, type BootstrapResult } from "./result.ts";
import { pluginBootstrapScript } from "./entrypoint.ts";
import { evaluateBootstrapReadiness } from "./readiness.ts";

export type BootstrapRuntimeOptions = {
  repoRoot: string;
  dataRoot?: string;
  projectRoot: string;
  plugins: PluginManifest[];
  env?: Record<string, string>;
  runner?: BashRunner;
  isolatedHome?: string;
  deadlineMs?: number;
};

function bootstrapEnv(options: BootstrapRuntimeOptions, dataRoot: string): Record<string, string> {
  const base: Record<string, string> = {
    TOOLU_CONFIG_DIR: dataRoot,
    TOOLU_PROJECT_DIR: options.projectRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_HOST_OVERRIDE: "opencode",
    HOME: options.isolatedHome ?? dataRoot,
  };
  if (options.env) {
    for (const [key, value] of Object.entries(options.env)) {
      base[key] = value;
    }
  }
  return base;
}

async function runEntrypoint(
  runner: BashRunner,
  scriptPath: string,
  cwd: string,
  env: Record<string, string>,
  deadlineMs: number,
): Promise<BootstrapResult | null> {
  const result = await runner.run({
    argv: ["bash", scriptPath],
    cwd,
    env,
    stdin: "{}",
    deadlineMs,
    maxStdoutBytes: 512_000,
  });
  if (!result.ok) {
    return notReady(`bootstrap script failed (${scriptPath}): ${result.message}`);
  }
  if (result.exitCode !== 0) {
    return notReady(
      `bootstrap script ${scriptPath} exited ${result.exitCode}: ${result.stderr || result.stdout}`,
    );
  }
  return null;
}

/** Run plugin register/session-start scripts and verify output artifacts. */
export async function bootstrapRuntime(options: BootstrapRuntimeOptions): Promise<BootstrapResult> {
  const dataRoot = options.dataRoot ?? opencodeDataRoot({ projectRoot: options.projectRoot });
  mkdirSync(dataRoot, { recursive: true });
  mkdirSync(dirname(dataRoot), { recursive: true });

  const runner = options.runner ?? createBunBashRunner();
  const env = bootstrapEnv(options, dataRoot);
  const deadlineMs = options.deadlineMs ?? 120_000;

  const runChain = async (index: number): Promise<BootstrapResult | null> => {
    const plugin = options.plugins[index];
    if (!plugin) {
      return null;
    }
    const script = pluginBootstrapScript(plugin.pluginDir);
    if (!script) {
      return notReady(`no bootstrap entrypoint for plugin ${plugin.spec}`);
    }
    const failure = await runEntrypoint(runner, script, options.projectRoot, env, deadlineMs);
    if (failure) {
      return failure;
    }
    return runChain(index + 1);
  };

  const chainFailure = await runChain(0);
  if (chainFailure) {
    return chainFailure;
  }

  const proof = evaluateBootstrapReadiness(dataRoot);
  if (!proof.ready) {
    return notReady(proof.reason ?? "bootstrap artifacts missing");
  }
  return ready(proof.artifacts);
}
