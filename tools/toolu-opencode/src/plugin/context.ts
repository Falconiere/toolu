/** Bind the documented `PluginInput` and plugin options to what toolu needs (#336). */
import type { PluginInput, PluginOptions } from "@opencode-ai/plugin";
import { z } from "zod";
import { definedEnv } from "../host/runtime-env.ts";

export type LogLevel = "info" | "error";

/** Flat structured fields of one host-log entry (#359), sent as the SDK's `body.extra`. */
export type LogExtra = Record<string, string | number>;

export type HostBinding = {
  /** The host instance directory: the cwd every gate runs in. */
  directory: string;
  /** The host's own worktree, `/` outside version control; skill discovery walks up to it. */
  worktree: string;
  /** Where `.opencode/toolu.config.json` and `.opencode/toolu/plugins.json` live. */
  projectRoot: string;
  repoRootOption: string | undefined;
  /** Set when the plugin options fail validation; enforcement then refuses every call. */
  optionsError: string | undefined;
  env: Record<string, string>;
  log: (level: LogLevel, message: string, extra?: LogExtra) => Promise<void>;
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

/** Send one entry to the host log; a failed or slow call is dropped, never thrown. */
function hostLog(client: PluginInput["client"], timeoutMs: number): HostBinding["log"] {
  return async (level, message, extra) => {
    const body = { service: "toolu", level, message, ...(extra === undefined ? {} : { extra }) };
    const sent = client.app.log({ body }).then(
      () => undefined,
      () => undefined,
    );
    const timeout = new Promise<void>((resolve) => {
      setTimeout(resolve, timeoutMs).unref();
    });
    await Promise.race([sent, timeout]);
  };
}

export function bindHostContext(
  input: Pick<PluginInput, "client" | "directory" | "worktree">,
  options: PluginOptions | undefined,
  env: NodeJS.ProcessEnv,
  logTimeoutMs = LOG_TIMEOUT_MS,
): HostBinding {
  return {
    directory: input.directory,
    worktree: input.worktree,
    projectRoot: projectRootOf(input.worktree, input.directory),
    ...parseOptions(options),
    env: definedEnv(env),
    log: hostLog(input.client, logTimeoutMs),
  };
}
