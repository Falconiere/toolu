/**
 * Deprecation notice (#403): one release of warning before #406 removes
 * exa-search, context7, jira and agent-browser. The line is a `systemMessage`,
 * shown to the user at SessionStart and never injected into model context, and
 * names the uninstall command of the host running the hook.
 */
import { resolveHost, pluginUninstallCommand, type HostOptions } from "../host/host-roots.ts";
import { renderHookOutput, type SessionContext } from "./context.ts";

/** The release that removes the deprecated plugins: #406 is `feat!`, so the next major. */
export const REMOVAL_RELEASE = "v8.0.0";

/** `<plugin> is deprecated and will be removed in v8.0.0; uninstall with: <command>`. */
export function deprecationNotice(plugin: string, options: HostOptions = {}): string {
  const command = pluginUninstallCommand(plugin, options);
  const how =
    command === null
      ? "uninstall it with your host's plugin manager"
      : `uninstall with: ${command}`;
  return `${plugin} is deprecated and will be removed in ${REMOVAL_RELEASE}; ${how}`;
}

/**
 * The deprecated plugin's whole SessionStart stdout: `context` plus the notice
 * as `systemMessage`, in one compact object. OpenCode re-runs SessionStart on
 * every compaction, so there the notice is left out when `compacting`, and
 * shows once per host start; `context` alone remains ("" when absent).
 */
export function deprecatedStartupOutput(
  plugin: string,
  context: SessionContext | undefined,
  options: HostOptions & { compacting?: boolean } = {},
): string {
  const resolved = resolveHost(options);
  if (resolved.host === "opencode" && options.compacting === true) {
    return context === undefined ? "" : renderHookOutput(context, false);
  }
  return renderHookOutput(
    { ...context, systemMessage: deprecationNotice(plugin, resolved) },
    false,
  );
}

/** SessionStart stdin names its trigger; unreadable stdin counts as a plain start. */
export async function startedByCompaction(
  stdin: Promise<string> = Bun.stdin.text(),
): Promise<boolean> {
  let input: unknown;
  try {
    input = JSON.parse(await stdin);
  } catch {
    return false;
  }
  return (
    input !== null && typeof input === "object" && "source" in input && input.source === "compact"
  );
}
