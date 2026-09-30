/**
 * SessionStart (#266): copy the python-quality registry module into the
 * host's `post-tools.d` as `python-quality@toolu__python-quality.js`, and drop
 * this plugin's older entries, including the assembled bash module it replaces.
 */
import { join } from "node:path";
import { runRegisterHook } from "@toolu/core/registry";

await runRegisterHook("python-quality@toolu", [
  { name: "python-quality", event: "tool/post", bundle: join(import.meta.dir, "post-tool-use.js") },
]);
