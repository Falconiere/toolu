/** exa-search's SessionStart bundle through its real hooks.json launcher (#269, ported from session-start.bats). */
import { resolve } from "node:path";
import { join } from "node:path";
import { readlinkSync } from "node:fs";
import { expect, test } from "bun:test";
import { publishedCliSuite } from "@toolu/conformance/harness/startup";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { runStartupHook } from "@toolu/conformance/harness/startup";

const PLUGIN_ROOT = resolve(import.meta.dir, "../../..");

publishedCliSuite({
  plugin: "exa-search",
  pluginRoot: PLUGIN_ROOT,
  source: "hooks/dist/search.js",
  dir: "exa-search",
  name: "search.sh",
  advisory:
    "exa-search: bun not found on PATH — the exa-search search CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: { EXA_API_KEY: "exa-test" },
  probe: { args: [], env: { EXA_API_KEY: "k" }, exitCode: 1, output: "Exa Search CLI" },
  notice: {
    claude:
      "exa-search is deprecated and will be removed in v8.0.0; uninstall with: claude plugin uninstall exa-search@toolu",
    codex:
      "exa-search is deprecated and will be removed in v8.0.0; uninstall with: codex plugin remove exa-search@toolu",
    opencode:
      "exa-search is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove exa-search --host opencode --yes",
  },
});

for (const key of ["sentinel-exa-key", undefined]) {
  test(`OpenCode startup publishes helper and ${key === undefined ? "no-key fallback" : "research guidance"}`, async () => {
    using sb = createSandbox();
    const dataRoot = join(sb.project, ".opencode/toolu/state");
    const env = {
      HOME: sb.home,
      CLAUDE_PLUGIN_ROOT: PLUGIN_ROOT,
      TOOLU_CONFIG_DIR: dataRoot,
      TOOLU_HOST_OVERRIDE: "opencode",
      EXA_API_KEY: key,
    };
    const result = await runStartupHook(PLUGIN_ROOT, "session-start", sb, env);
    expect(result.exitCode).toBe(0);
    expect(result.stderr).toBe("");
    const published = join(dataRoot, "exa-search/search.sh");
    expect(readlinkSync(published)).toBe(join(PLUGIN_ROOT, "hooks/dist/search.js"));
    const parsed = JSON.parse(result.stdout) as {
      hookSpecificOutput: { hookEventName: string; additionalContext: string };
    };
    expect(parsed.hookSpecificOutput.hookEventName).toBe("SessionStart");
    const context = parsed.hookSpecificOutput.additionalContext;
    expect(context).toContain(published);
    expect(context).toContain("exa-search-exa-search");
    expect(context).not.toContain("sentinel-exa-key");
    if (key === undefined) {
      expect(context).toContain("EXA_API_KEY");
      expect(context).toContain("websearch");
      expect(context).toContain("webfetch");
    } else {
      expect(context).toContain("search");
      expect(context).toContain("crawl");
      expect(context).toContain("similar");
    }
  });
}
