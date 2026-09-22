/** OpenCode plugin entry — bootstrap + permission.evaluate bridge (#204). */
import { Plugin } from "@opencode/plugin";
import { join } from "node:path";
import { bootstrapRuntime } from "../bootstrap/runtime.ts";
import { runPreflight } from "../preflight/check.ts";
import {
  createDenyAllPermissionHandler,
  createPermissionEvaluateHandler,
} from "../adapter/evaluate.ts";
import { selectPluginsWithDependencies } from "../select/resolve.ts";
import { opencodeDataRoot } from "../host/roots.ts";

function readNonEmptyString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function repoRootFromOptions(
  options: Readonly<Record<string, unknown>>,
  env: Record<string, string>,
): string | undefined {
  return (
    readNonEmptyString(options.repoRoot) ??
    readNonEmptyString(env.TOOLU_REPO_ROOT) ??
    readNonEmptyString(env.TOOLU_ROOT)
  );
}

export type SetupTooluPermissionOptions = {
  repoRoot: string;
  projectRoot: string;
  env?: Record<string, string>;
};

/** Wire permission.evaluate after preflight + bootstrap; deny-all when NotReady. */
export async function setupTooluPermissionHook(
  ctx: Pick<Plugin.Context, "permission" | "location">,
  setupOpts: SetupTooluPermissionOptions,
): Promise<void> {
  const env = setupOpts.env ?? {};
  const preflight = runPreflight({ env });
  if (!preflight.bootstrapAllowed) {
    const reason = preflight.reasons.join("; ") || "preflight failed";
    await ctx.permission.hook(
      "evaluate",
      createDenyAllPermissionHandler(`toolu preflight: ${reason}`),
    );
    return;
  }

  const pluginsRoot = join(setupOpts.repoRoot, "plugins");
  const selected = selectPluginsWithDependencies(pluginsRoot, setupOpts.projectRoot);
  if (!selected.ok) {
    await ctx.permission.hook(
      "evaluate",
      createDenyAllPermissionHandler(`toolu plugin selection: ${selected.reason}`),
    );
    return;
  }

  const dataRoot = opencodeDataRoot({
    projectRoot: setupOpts.projectRoot,
    env,
  });

  const bootstrap = await bootstrapRuntime({
    repoRoot: setupOpts.repoRoot,
    projectRoot: setupOpts.projectRoot,
    dataRoot,
    plugins: selected.plugins,
    env,
  });

  if (bootstrap.status !== "ready") {
    await ctx.permission.hook(
      "evaluate",
      createDenyAllPermissionHandler(`toolu bootstrap: ${bootstrap.reason}`),
    );
    return;
  }

  const bridgeContext = {
    cwd: ctx.location.directory,
    projectRoot: setupOpts.projectRoot,
    worktree: ctx.location.directory,
    host: "opencode",
  };

  const handler = createPermissionEvaluateHandler({
    repoRoot: setupOpts.repoRoot,
    bridgeContext,
    env: {
      ...env,
      TOOLU_SETTINGS_DIR: join(setupOpts.repoRoot, "plugins/toolu/settings"),
      TOOLU_HOST_OVERRIDE: "opencode",
    },
  });

  await ctx.permission.hook("evaluate", handler);
}

export default Plugin.define({
  id: "toolu",
  async setup(ctx) {
    const env: Record<string, string> = {};
    for (const [key, value] of Object.entries(process.env)) {
      if (value !== undefined) {
        env[key] = value;
      }
    }

    const optionsRecord: Record<string, unknown> = { ...ctx.options };
    const repoRoot = repoRootFromOptions(optionsRecord, env);
    if (!repoRoot) {
      await ctx.permission.hook(
        "evaluate",
        createDenyAllPermissionHandler(
          "toolu: set plugin option repoRoot or TOOLU_REPO_ROOT for gate enforcement",
        ),
      );
      return;
    }

    const projectRoot = ctx.location.project.directory;
    await setupTooluPermissionHook(ctx, {
      repoRoot,
      projectRoot,
      env,
    });
  },
});
