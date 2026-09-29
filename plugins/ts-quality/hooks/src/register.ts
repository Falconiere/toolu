/**
 * SessionStart (#265): copy the ts-quality registry module into the host's
 * `post-tools.d` as `ts-quality@toolu__ts-quality.js`, and drop this plugin's
 * older entries, including the assembled bash module it replaces.
 */
import { join } from "node:path";
import { runRegisterHook } from "@toolu/core/registry";

await runRegisterHook("ts-quality@toolu", [
  { name: "ts-quality", event: "tool/post", bundle: join(import.meta.dir, "post-tool-use.js") },
]);
