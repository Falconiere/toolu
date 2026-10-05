/** toolu-review's SessionStart bundle through its real hooks.json launcher (#269, ported from session-start.bats). */
import { resolve } from "node:path";
import { bundlePath } from "@toolu/conformance/harness/entry-command";
import { publishedCliSuite } from "@toolu/conformance/harness/startup";

publishedCliSuite({
  plugin: "toolu-review",
  pluginRoot: resolve(import.meta.dir, "../../.."),
  source: bundlePath("", "write-state"),
  dir: "toolu-review",
  name: "write-state.sh",
  advisory:
    "toolu-review: bun not found on PATH — the toolu-review write-state helper needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: {},
  probe: { args: [], exitCode: 2, output: "write-state.sh: --findings-count required" },
});
