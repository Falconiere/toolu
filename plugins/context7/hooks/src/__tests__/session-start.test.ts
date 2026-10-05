/** context7's SessionStart bundle through its real hooks.json launcher (#269, ported from session-start.bats). */
import { resolve } from "node:path";
import { bundlePath } from "@toolu/conformance/harness/entry-command";
import { publishedCliSuite } from "@toolu/conformance/harness/startup";

publishedCliSuite({
  plugin: "context7",
  pluginRoot: resolve(import.meta.dir, "../../.."),
  source: bundlePath("", "search"),
  dir: "context7",
  name: "search.sh",
  advisory:
    "context7: bun not found on PATH — the context7 search CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: { CONTEXT7_API_KEY: "ctx7sk-test" },
  probe: { args: [], exitCode: 1, output: "Context7 CLI" },
  notice: {
    claude:
      "context7 is deprecated and will be removed in v8.0.0; uninstall with: claude plugin uninstall context7@toolu",
    codex:
      "context7 is deprecated and will be removed in v8.0.0; uninstall with: codex plugin remove context7@toolu",
    opencode:
      "context7 is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove context7 --host opencode --yes",
  },
});
