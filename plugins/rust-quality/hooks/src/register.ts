/**
 * SessionStart (#267): copy the rust-quality registry module into the host's
 * `post-tools.d` as `rust-quality@toolu__rust-quality.js`, and drop this
 * plugin's older entries, including the assembled bash module it replaces.
 */
import { join } from "node:path";
import { runRegisterHook } from "@toolu/core/registry";

await runRegisterHook("rust-quality@toolu", [
  { name: "rust-quality", event: "tool/post", bundle: join(import.meta.dir, "post-tool-use.js") },
]);
