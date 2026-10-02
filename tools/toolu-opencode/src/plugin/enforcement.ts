/** Preflight, plugin selection and bootstrap, then the gate hook — or the reason toolu is not ready (#336). */
import { existsSync } from "node:fs";
import { join } from "node:path";
import { createToolBeforeHandler, type ToolBefore } from "../adapter/tool-before.ts";
import { bootstrapRuntime } from "../bootstrap/runtime.ts";
import { opencodeDataRoot } from "../host/roots.ts";
import { runPreflight } from "../preflight/check.ts";
import { selectPluginsWithDependencies } from "../select/resolve.ts";
import type { HostBinding } from "./context.ts";

export type Enforcement =
  | { status: "ready"; before: ToolBefore; artifacts: string[] }
  | { status: "not-ready"; reason: string };

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

export function resolveRepoRoot(binding: HostBinding): string | undefined {
  return (
    binding.repoRootOption ??
    nonEmpty(binding.env.TOOLU_REPO_ROOT) ??
    nonEmpty(binding.env.TOOLU_ROOT) ??
    bundledRepoRoot()
  );
}

export async function prepareEnforcement(binding: HostBinding): Promise<Enforcement> {
  if (binding.optionsError !== undefined) return notReady(binding.optionsError);
  const repoRoot = resolveRepoRoot(binding);
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
  });
  return { status: "ready", before, artifacts: bootstrap.artifacts };
}
