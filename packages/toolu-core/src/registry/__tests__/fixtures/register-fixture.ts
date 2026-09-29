/**
 * A plugin's SessionStart register entry as the registry tests bundle it
 * (#257). A real plugin names its own committed bundles relative to
 * `import.meta.dir`; the test passes the bundle path in instead.
 */
import { runRegisterHook } from "../../registry-register.ts";

await runRegisterHook("fixture@toolu", [
  { name: "fixture", event: "tool/post", bundle: process.env.FIXTURE_BUNDLE ?? "" },
]);
