/** The deprecation notice (#403): exact line and stdout per host, from options and real env detection. */
import { describe, expect, test } from "bun:test";
import { pluginUninstallCommand } from "../../host/host-roots.ts";
import { sessionContext } from "../context.ts";
import {
  REMOVAL_RELEASE,
  deprecatedStartupOutput,
  deprecationNotice,
  startedByCompaction,
} from "../deprecation.ts";

const PREFIX = "context7 is deprecated and will be removed in v8.0.0;";

describe("pluginUninstallCommand", () => {
  test("names each host's own uninstall command, null where none exists", () => {
    expect(pluginUninstallCommand("context7", { env: {}, host: "claude" })).toBe(
      "claude plugin uninstall context7@toolu",
    );
    expect(pluginUninstallCommand("jira", { env: {}, host: "codex" })).toBe(
      "codex plugin remove jira@toolu",
    );
    expect(pluginUninstallCommand("agent-browser", { env: {}, host: "opencode" })).toBe(
      "npx @toolu/plugins remove agent-browser --host opencode --yes",
    );
    for (const host of ["cursor", "hermes"] as const) {
      expect(pluginUninstallCommand("exa-search", { env: {}, host })).toBeNull();
    }
    expect(() => pluginUninstallCommand("", { env: {} })).toThrow(TypeError);
  });
});

describe("deprecationNotice", () => {
  test("names the removal release and the detected host's command", () => {
    expect(REMOVAL_RELEASE).toBe("v8.0.0");
    const cases: [Record<string, string>, string][] = [
      [{}, "claude plugin uninstall context7@toolu"],
      [{ PLUGIN_ROOT: "/p" }, "codex plugin remove context7@toolu"],
      [
        { TOOLU_HOST_OVERRIDE: "opencode" },
        "npx @toolu/plugins remove context7 --host opencode --yes",
      ],
    ];
    for (const [env, command] of cases) {
      expect(deprecationNotice("context7", { env })).toBe(`${PREFIX} uninstall with: ${command}`);
    }
  });

  test("falls back to a generic sentence on hosts without a command", () => {
    for (const env of [{ CURSOR_VERSION: "1.0" }, { TOOLU_HOST_OVERRIDE: "hermes" }]) {
      expect(deprecationNotice("context7", { env })).toBe(
        `${PREFIX} uninstall it with your host's plugin manager`,
      );
    }
  });
});

describe("deprecatedStartupOutput", () => {
  const context = sessionContext("SessionStart", "helper guidance");

  test("Claude and Codex: one compact systemMessage object, never additionalContext", () => {
    for (const compacting of [false, true]) {
      const out = deprecatedStartupOutput("jira", undefined, { env: {}, compacting });
      expect(out).toBe(
        '{"systemMessage":"jira is deprecated and will be removed in v8.0.0; uninstall with: claude plugin uninstall jira@toolu"}\n',
      );
    }
    const codex = JSON.parse(
      deprecatedStartupOutput("jira", undefined, { env: { PLUGIN_ROOT: "/p" } }),
    );
    expect(codex).toEqual({
      systemMessage:
        "jira is deprecated and will be removed in v8.0.0; uninstall with: codex plugin remove jira@toolu",
    });
  });

  test("OpenCode start: the context keeps its text and gains the notice beside it", () => {
    const env = { TOOLU_HOST_OVERRIDE: "opencode" };
    const parsed = JSON.parse(deprecatedStartupOutput("exa-search", context, { env }));
    expect(parsed).toEqual({
      hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: "helper guidance" },
      systemMessage:
        "exa-search is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove exa-search --host opencode --yes",
    });
  });

  test("OpenCode compaction: the context alone, or nothing", () => {
    const options = { env: { TOOLU_HOST_OVERRIDE: "opencode" }, compacting: true };
    expect(deprecatedStartupOutput("exa-search", context, options)).toBe(
      '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"helper guidance"}}\n',
    );
    expect(deprecatedStartupOutput("context7", undefined, options)).toBe("");
  });
});

describe("startedByCompaction", () => {
  test("true only for a compact source; unreadable stdin is a plain start", async () => {
    const cases: [string, boolean][] = [
      ['{"source":"compact"}', true],
      ['{"source":"startup"}', false],
      ["{}", false],
      ["null", false],
      ["", false],
      ["not json", false],
    ];
    for (const [stdin, expected] of cases) {
      expect(await startedByCompaction(Promise.resolve(stdin))).toBe(expected);
    }
  });
});
