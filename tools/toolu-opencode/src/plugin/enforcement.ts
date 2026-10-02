/** Preflight, plugin selection and bootstrap, then the gate hook — or the reason toolu is not ready (#336). */
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { createToolAdviceStore } from "../adapter/tool-advice.ts";
import { createToolBeforeHandler, type ToolBefore } from "../adapter/tool-before.ts";
import type { PluginStartup } from "../bootstrap/result.ts";
import { bootstrapRuntime } from "../bootstrap/runtime.ts";
import { opencodeConfigRoot, opencodeDataRoot } from "../host/roots.ts";
import { shellEnvFor, type OpencodeRoots, type ShellEnv } from "../host/runtime-env.ts";
import { resolveBunExecutable, runPreflight } from "../preflight/check.ts";
import { selectPluginsWithDependencies } from "../select/resolve.ts";
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
  const roots: OpencodeRoots = {
    projectRoot,
    dataRoot: opencodeDataRoot({ projectRoot, env }),
    userConfigRoot: opencodeConfigRoot({ env }),
    repoRoot,
    packageRoot: PACKAGE_ROOT,
  };
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
  const { artifacts, plugins, diagnostics } = bootstrap;
  return {
    status: "ready",
    before,
    after: advice.after,
    clearAdvice: advice.clear,
    artifacts,
    plugins,
    diagnostics,
    shellEnv,
  };
}
