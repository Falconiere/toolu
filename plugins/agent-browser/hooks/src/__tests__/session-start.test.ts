/** agent-browser's SessionStart bundle through its real hooks.json launcher (#269, ported from session-start.bats). */
import { resolve } from "node:path";
import { readlinkSync } from "node:fs";
import { expect, test } from "bun:test";
import { bundlePath } from "@toolu/conformance/harness/entry-command";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { publishedCliSuite, runStartupHook } from "@toolu/conformance/harness/startup";

const pluginRoot = resolve(import.meta.dir, "../../..");

publishedCliSuite({
  plugin: "agent-browser",
  pluginRoot,
  source: bundlePath("", "agent-browser"),
  dir: "agent-browser",
  name: "agent-browser.sh",
  advisory:
    "agent-browser: bun not found on PATH — the agent-browser wrapper needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  credentials: {},
  probe: {
    args: ["snapshot"],
    env: { AGENT_BROWSER_BIN: "/nonexistent/agent-browser" },
    exitCode: 127,
    output: "agent-browser not found",
  },
  notice: {
    claude:
      "agent-browser is deprecated and will be removed in v8.0.0; uninstall with: claude plugin uninstall agent-browser@toolu",
    codex:
      "agent-browser is deprecated and will be removed in v8.0.0; uninstall with: codex plugin remove agent-browser@toolu",
    opencode:
      "agent-browser is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove agent-browser --host opencode --yes",
  },
});

test("OpenCode startup names the published helper and native browser skill", async () => {
  using sb = createSandbox();
  const dataRoot = resolve(sb.project, ".opencode/toolu/state");
  const result = await runStartupHook(pluginRoot, "session-start", sb, {
    HOME: sb.home,
    CLAUDE_PLUGIN_ROOT: pluginRoot,
    TOOLU_BUN: process.execPath,
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: dataRoot,
  });
  expect(result.exitCode).toBe(0);
  expect(result.stderr).toBe("");
  const output = JSON.parse(result.stdout);
  expect(output.hookSpecificOutput.hookEventName).toBe("SessionStart");
  expect(output.hookSpecificOutput.additionalContext).toContain(
    `${dataRoot}/agent-browser/agent-browser.sh`,
  );
  expect(output.hookSpecificOutput.additionalContext).toContain("agent-browser-agent-browser");
  expect(output.hookSpecificOutput.additionalContext).toContain("snapshot");
  expect(readlinkSync(`${dataRoot}/agent-browser/agent-browser.sh`)).toBe(
    bundlePath(pluginRoot, "agent-browser"),
  );
});
