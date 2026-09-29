/** context7's SessionStart bundle through its real hooks.json launcher (#269, ported from session-start.bats). */
import { resolve } from "node:path";
import { publishedCliSuite } from "@toolu/conformance/harness/startup";

publishedCliSuite({
  plugin: "context7",
  pluginRoot: resolve(import.meta.dir, "../../.."),
  source: "hooks/dist/search.js",
  dir: "context7",
  name: "search.sh",
  advisory:
    "context7: bun not found on PATH — the context7 search CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: { CONTEXT7_API_KEY: "ctx7sk-test" },
  probe: { args: [], exitCode: 1, output: "Context7 CLI" },
});
