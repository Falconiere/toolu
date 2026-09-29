/**
 * What toolu's SessionStart does before any context is built (#263), in
 * session-start.sh's order: on Codex, refresh the plugin snapshot and prune
 * the registry against it (once per session start), drop the legacy statusline
 * symlink, and write the readiness marker the OpenCode bootstrap waits for.
 * These run even when the hook's context is disabled.
 */
import { lstatSync, rmSync } from "node:fs";
import { join } from "node:path";
import { snapshotCodexPlugins, type HostName } from "@toolu/core/host";
import { pruneInactiveModules } from "@toolu/core/registry";
import { touch } from "./session-notices.ts";

type Env = Record<string, string | undefined>;

function isSymlink(path: string): boolean {
  try {
    return lstatSync(path).isSymbolicLink();
  } catch {
    return false;
  }
}

export function housekeeping(env: Env, host: HostName, configRoot: string): void {
  if (host === "codex") {
    snapshotCodexPlugins({ env, host });
    pruneInactiveModules({ env, host });
  }
  const legacy = join(configRoot, "toolu", "statusline.sh");
  if (isSymlink(legacy)) {
    try {
      rmSync(legacy, { force: true });
    } catch {
      // `rm -f` parity: an unremovable link must not cost the readiness marker below.
    }
  }
  touch(join(configRoot, "toolu", ".session-start-ready"));
}
