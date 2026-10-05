/**
 * Deprecation banners (#403): every page a user reads about a plugin #406
 * removes opens with the same notice the plugin's SessionStart prints: the
 * removal release, the replacement and each host's uninstall command.
 */
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pluginUninstallCommand } from "@toolu/core/host";
import { REMOVAL_RELEASE } from "@toolu/core/startup";

const ROOT = resolve(import.meta.dir, "../../..");

const REPLACEMENT: Readonly<Record<string, string>> = {
  "exa-search": "Use your host's native web search and fetch tools instead.",
  context7: "Use your host's native web search and fetch tools instead.",
  jira: "For Jira epics, use epic-orchestrator's built-in Jira tracker instead.",
  "agent-browser": "It has no replacement.",
};

const PAGES: readonly [string, string][] = [
  ...Object.keys(REPLACEMENT).map((plugin): [string, string] => [
    plugin,
    `plugins/${plugin}/README.md`,
  ]),
  ["exa-search", "docs/exa-search/README.md"],
  ["context7", "docs/context7/README.md"],
  ["jira", "docs/jira/README.md"],
  ["agent-browser", "plugins/agent-browser/skills/agent-browser/SKILL.md"],
  ["agent-browser", "tools/toolu-opencode/generated/skills/agent-browser-agent-browser/SKILL.md"],
];

/** The first non-empty line after the page's H1. */
function lineAfterTitle(text: string): string {
  const lines = text.split("\n");
  const title = lines.findIndex((line) => line.startsWith("# "));
  return lines.slice(title + 1).find((line) => line.trim() !== "") ?? "";
}

for (const [plugin, page] of PAGES) {
  test(`${page} opens with the ${plugin} deprecation banner`, () => {
    const banner = lineAfterTitle(readFileSync(resolve(ROOT, page), "utf8"));
    expect(banner).toStartWith(
      `> **Deprecated:** ${plugin} will be removed in ${REMOVAL_RELEASE}. ${REPLACEMENT[plugin]}`,
    );
    for (const host of ["claude", "codex", "opencode"] as const) {
      const command = pluginUninstallCommand(plugin, { env: {}, host });
      expect(banner).toContain(`\`${command}\``);
    }
  });
}
