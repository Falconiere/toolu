/** Locate committed Bun SessionStart and register bundles. */
import { existsSync } from "node:fs";
import { join } from "node:path";

export function pluginBootstrapScript(pluginDir: string): string | null {
  const candidates = [
    join(pluginDir, "hooks", "dist", "register.js"),
    join(pluginDir, "hooks", "dist", "session-start.js"),
  ];
  return candidates.find((path) => existsSync(path)) ?? null;
}

/** An unported registration hook is never silently reported as bootstrapped. */
export function requiresNativeRegistration(pluginDir: string): boolean {
  const hooks = join(pluginDir, "hooks");
  if (existsSync(join(hooks, ".requires-native-register"))) return true;
  const hasRegisterBundle = existsSync(join(hooks, "dist", "register.js"));
  if (!hasRegisterBundle && existsSync(join(hooks, "register.sh"))) return true;
  return (
    !hasRegisterBundle &&
    !existsSync(join(hooks, "dist", "session-start.js")) &&
    existsSync(join(hooks, "session-start.sh"))
  );
}
