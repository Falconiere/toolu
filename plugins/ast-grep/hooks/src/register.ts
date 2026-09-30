/**
 * SessionStart (#268): copy ast-grep's two registry modules into the host's
 * registry as `ast-grep@toolu__search-nudge.js` (pre-tools.d) and
 * `ast-grep@toolu__byte-savings.js` (post-tools.d), and drop this plugin's
 * older entries, including the bash modules they replace.
 */
import { join } from "node:path";
import { runRegisterHook } from "@toolu/core/registry";

await runRegisterHook("ast-grep@toolu", [
  { name: "search-nudge", event: "tool/pre", bundle: join(import.meta.dir, "search-nudge.js") },
  { name: "byte-savings", event: "tool/post", bundle: join(import.meta.dir, "byte-savings.js") },
]);
