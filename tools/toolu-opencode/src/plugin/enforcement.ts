/** Preflight, plugin selection and bootstrap, then the gate hook — or the reason toolu is not ready (#336). */
import { existsSync } from "node:fs";
import { join } from "node:path";
import { createToolBeforeHandler, type ToolBefore } from "../adapter/tool-before.ts";
import type { PluginStartup } from "../bootstrap/result.ts";
import { bootstrapRuntime } from "../bootstrap/runtime.ts";
import { opencodeDataRoot } from "../host/roots.ts";
import { runPreflight } from "../preflight/check.ts";
import { selectPluginsWithDependencies } from "../select/resolve.ts";
import type { HostBinding } from "./context.ts";

export type Enforcement =
  | {
      status: "ready";
      before: ToolBefore;
      artifacts: string[];
      /** Each selected plugin's startup, with the context OP-07 delivers. */
      plugins: PluginStartup[];
      diagnostics: string[];
    }
  | { status: "not-ready"; reason: string };

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
  const packageRoot = join(import.meta.dir, "../..");
  return existsSync(join(packageRoot, "plugins")) ? packageRoot : undefined;
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
  const repoRoot = resolveRepoRoot(binding, findBundled);
  if (repoRoot === undefined)
    return notReady("no bundled plugins/ tree; set plugin option repoRoot or TOOLU_REPO_ROOT");
  const { env, projectRoot } = binding;
  const preflight = runPreflight({ env });
  if (!preflight.bootstrapAllowed)
    return notReady(`preflight: ${preflight.reasons.join("; ") || "preflight failed"}`);
  const selected = selectPluginsWithDependencies(join(repoRoot, "plugins"), projectRoot);
  if (!selected.ok) return notReady(`plugin selection: ${selected.reason}`);
  const dataRoot = opencodeDataRoot({ projectRoot, env });
  const bootstrap = await bootstrapRuntime({
    repoRoot,
    projectRoot,
    dataRoot,
    plugins: selected.plugins,
    env,
    signal: AbortSignal.timeout(STARTUP_BUDGET_MS),
  });
  if (bootstrap.status !== "ready") return notReady(`bootstrap: ${bootstrap.reason}`);
  const before = createToolBeforeHandler({
    repoRoot,
    configRoot: dataRoot,
    permissionContext: { cwd: binding.directory, projectRoot, worktree: projectRoot },
    env: {
      ...env,
      TOOLU_SETTINGS_DIR: join(repoRoot, "plugins/toolu/settings"),
      TOOLU_HOST_OVERRIDE: "opencode",
    },
    selectedPluginSpecs: new Set(selected.plugins.map((plugin) => plugin.spec)),
  });
  const { artifacts, plugins, diagnostics } = bootstrap;
  return { status: "ready", before, artifacts, plugins, diagnostics };
}
