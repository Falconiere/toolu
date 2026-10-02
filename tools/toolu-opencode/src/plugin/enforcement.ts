/** Preflight, plugin selection and bootstrap, then the gate hook — or the reason toolu is not ready (#336). */
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { createToolAdviceStore } from "../adapter/tool-advice.ts";
import { createToolBeforeHandler, type ToolBefore } from "../adapter/tool-before.ts";
import type { PluginStartup } from "../bootstrap/result.ts";
import type { PluginManifest } from "../inventory/types.ts";
import { bootstrapRuntime } from "../bootstrap/runtime.ts";
import { opencodeConfigRoot, opencodeDataRoot } from "../host/roots.ts";
import {
  shellEnvFor,
  tooluProcessEnv,
  type OpencodeRoots,
  type ShellEnv,
} from "../host/runtime-env.ts";
import { resolveBunExecutable, runPreflight } from "../preflight/check.ts";
import { selectPluginsWithDependencies } from "../select/resolve.ts";
import { contextJobs, type ContextPlan } from "./context-delivery.ts";
import type { HostBinding } from "./context.ts";

export type Enforcement =
  | {
      status: "ready";
      before: ToolBefore;
      after: ReturnType<typeof createToolAdviceStore>["after"];
      clearAdvice: () => void;
      artifacts: string[];
      /** Each selected plugin's startup, with the context OP-07 delivers. */
      plugins: PluginStartup[];
      diagnostics: string[];
      /** What `shell.env` adds to every bash call (#343). */
      shellEnv: ShellEnv;
      /** Startup lines and the bundles that produce prompt and compaction text (#341). */
      context: ContextPlan;
    }
  | { status: "not-ready"; reason: string };

/** This package's directory: it holds `generated/`, and `plugins/` when packed. */
const PACKAGE_ROOT = join(import.meta.dir, "../..");

/** The whole startup, every plugin and entry together, never blocks plugin init for longer. */
const STARTUP_BUDGET_MS = 180_000;

function notReady(reason: string): Enforcement {
  return { status: "not-ready", reason };
}

/**
 * The published package carries the bundled plugins/ catalog beside its sources, so an
 * npm install needs no environment variable. Absent that copy — a contributor
 * running from a clone — this returns undefined and the explicit sources win.
 */
function bundledRepoRoot(): string | undefined {
  return existsSync(join(PACKAGE_ROOT, "plugins")) ? PACKAGE_ROOT : undefined;
}

function nonEmpty(value: string | undefined): string | undefined {
  return value !== undefined && value.length > 0 ? value : undefined;
}

function deliveryPlan(
  started: readonly PluginStartup[],
  plugins: readonly PluginManifest[],
  bun: string,
  projectRoot: string,
  env: Record<string, string>,
  roots: OpencodeRoots,
): ContextPlan | string {
  const ordered = started.flatMap((entry) => {
    const plugin = plugins.find((candidate) => candidate.name === entry.plugin);
    return plugin === undefined ? [] : [{ name: plugin.name, pluginDir: plugin.pluginDir }];
  });
  const jobs = contextJobs(ordered);
  if (!jobs.ok) return jobs.reason;
  const startupLines: string[] = [];
  const notices: string[] = [];
  for (const plugin of started) {
    for (const entry of plugin.entries) {
      if (entry.additionalContext !== undefined) startupLines.push(entry.additionalContext);
      if (entry.systemMessage !== undefined) {
        notices.push(`toolu: ${plugin.plugin}/${entry.entry}: ${entry.systemMessage}`);
      }
    }
  }
  return {
    startupLines,
    notices,
    prompt: jobs.prompt,
    compact: jobs.compact,
    bun,
    projectRoot,
    env: { ...tooluProcessEnv(env, roots), TOOLU_BUN: bun, TOOLU_HOST_OVERRIDE: "opencode" },
  };
}

/** Plugin option, then `TOOLU_REPO_ROOT` / `TOOLU_ROOT`, then the catalog bundled in the package. */
export function resolveRepoRoot(
  binding: HostBinding,
  findBundled: () => string | undefined = bundledRepoRoot,
): string | undefined {
  return (
    binding.repoRootOption ??
    nonEmpty(binding.env.TOOLU_REPO_ROOT) ??
    nonEmpty(binding.env.TOOLU_ROOT) ??
    findBundled()
  );
}

function rootsFor(binding: HostBinding, repoRoot: string): OpencodeRoots {
  return {
    projectRoot: binding.projectRoot,
    dataRoot: opencodeDataRoot({ projectRoot: binding.projectRoot, env: binding.env }),
    userConfigRoot: opencodeConfigRoot({ env: binding.env }),
    repoRoot,
    packageRoot: PACKAGE_ROOT,
  };
}

export async function prepareEnforcement(
  binding: HostBinding,
  findBundled: () => string | undefined = bundledRepoRoot,
): Promise<Enforcement> {
  if (binding.optionsError !== undefined) return notReady(binding.optionsError);
  const found = resolveRepoRoot(binding, findBundled);
  if (found === undefined)
    return notReady("no bundled plugins/ tree; set plugin option repoRoot or TOOLU_REPO_ROOT");
  // Absolute, so every path handed to bash still resolves after the agent changes directory.
  const repoRoot = resolve(found);
  const { env, projectRoot } = binding;
  const preflight = runPreflight({ env });
  if (!preflight.bootstrapAllowed)
    return notReady(`preflight: ${preflight.reasons.join("; ") || "preflight failed"}`);
  const selected = selectPluginsWithDependencies(join(repoRoot, "plugins"), projectRoot);
  if (!selected.ok) return notReady(`plugin selection: ${selected.reason}`);
  const bun = resolveBunExecutable(env);
  if (bun === null)
    return notReady("Bun runtime not found, checked TOOLU_BUN, PATH and ~/.bun/bin/bun");
  const roots = rootsFor(binding, repoRoot);
  const bootstrap = await bootstrapRuntime({
    repoRoot,
    projectRoot,
    dataRoot: roots.dataRoot,
    userConfigRoot: roots.userConfigRoot,
    plugins: selected.plugins,
    env,
    signal: AbortSignal.timeout(STARTUP_BUDGET_MS),
  });
  if (bootstrap.status !== "ready") return notReady(`bootstrap: ${bootstrap.reason}`);
  const advice = createToolAdviceStore();
  const before = createToolBeforeHandler(
    {
      repoRoot,
      configRoot: roots.dataRoot,
      userConfigRoot: roots.userConfigRoot,
      permissionContext: { cwd: binding.directory, projectRoot, worktree: projectRoot },
      env: {
        ...env,
        TOOLU_SETTINGS_DIR: join(repoRoot, "plugins/toolu/settings"),
        TOOLU_HOST_OVERRIDE: "opencode",
      },
      selectedPluginSpecs: new Set(selected.plugins.map((plugin) => plugin.spec)),
    },
    advice,
  );
  const shellEnv = shellEnvFor({ roots, plugins: selected.plugins, bun, host: env });
  const context = deliveryPlan(bootstrap.plugins, selected.plugins, bun, projectRoot, env, roots);
  if (typeof context === "string") return notReady(`context: ${context}`);
  return {
    status: "ready",
    before,
    after: advice.after,
    clearAdvice: advice.clear,
    artifacts: bootstrap.artifacts,
    plugins: bootstrap.plugins,
    diagnostics: bootstrap.diagnostics,
    shellEnv,
    context,
  };
}
