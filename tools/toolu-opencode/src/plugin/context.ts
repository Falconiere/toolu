/** Bind the documented `PluginInput` and plugin options to what toolu needs (#336). */
import type { PluginInput, PluginOptions } from "@opencode-ai/plugin";
import { z } from "zod";

export type LogLevel = "info" | "error";

export type HostBinding = {
  /** The host instance directory: the cwd every gate runs in. */
  directory: string;
  /** Where `.opencode/toolu.config.json` and `.opencode/toolu/plugins.json` live. */
  projectRoot: string;
  repoRootOption: string | undefined;
  /** Set when the plugin options fail validation; enforcement then refuses every call. */
  optionsError: string | undefined;
  env: Record<string, string>;
  log: (level: LogLevel, message: string) => Promise<void>;
};

const TooluPluginOptionsSchema = z.looseObject({ repoRoot: z.string().min(1).optional() });

/** The diagnostic never blocks plugin init for longer than this. */
const LOG_TIMEOUT_MS = 5_000;

/** A non-VCS project gets the worktree `/` from the host; its root is the directory. */
export function projectRootOf(worktree: string, directory: string): string {
  return worktree === "/" ? directory : worktree;
}

export function parseOptions(
  options: PluginOptions | undefined,
): Pick<HostBinding, "repoRootOption" | "optionsError"> {
  const parsed = TooluPluginOptionsSchema.safeParse(options ?? {});
  if (parsed.success) return { repoRootOption: parsed.data.repoRoot, optionsError: undefined };
  return {
    repoRootOption: undefined,
    optionsError: `invalid plugin options: ${z.prettifyError(parsed.error)}`,
  };
}

export function definedEnv(env: NodeJS.ProcessEnv): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) out[key] = value;
  }
  return out;
}

/** Send one entry to the host log; a failed or slow call is dropped, never thrown. */
function hostLog(client: PluginInput["client"]): HostBinding["log"] {
  return async (level, message) => {
    const sent = client.app.log({ body: { service: "toolu", level, message } }).then(
      () => undefined,
      () => undefined,
    );
    const timeout = new Promise<void>((resolve) => {
      setTimeout(resolve, LOG_TIMEOUT_MS).unref();
    });
    await Promise.race([sent, timeout]);
  };
}

export function bindHostContext(
  input: Pick<PluginInput, "client" | "directory" | "worktree">,
  options: PluginOptions | undefined,
  env: NodeJS.ProcessEnv,
): HostBinding {
  return {
    directory: input.directory,
    projectRoot: projectRootOf(input.worktree, input.directory),
    ...parseOptions(options),
    env: definedEnv(env),
    log: hostLog(input.client),
  };
}
