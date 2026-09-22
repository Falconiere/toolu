/** Discover SessionStart / register entrypoints per plugin (#211). */
import { existsSync } from "node:fs";
import { join } from "node:path";

export function pluginBootstrapScript(pluginDir: string): string | null {
  const register = join(pluginDir, "hooks", "register.sh");
  if (existsSync(register)) {
    return register;
  }
  const sessionStart = join(pluginDir, "hooks", "session-start.sh");
  if (existsSync(sessionStart)) {
    return sessionStart;
  }
  return null;
}
