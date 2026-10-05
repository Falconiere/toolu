/**
 * The deprecation notice on OpenCode (#403): the real toolu and plugin bundles
 * through the adapter's hooks. Each deprecated plugin's SessionStart notice
 * reaches the host log once when OpenCode loads, through the startup channel,
 * and never again on a tool call or a compaction; none reaches the model.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { createTooluHooks } from "../hooks.ts";
import { binding, hook, systemLines, type Logged } from "./jev-fixtures.ts";
import { bash, refusal } from "./workflow-fixtures.ts";

const DEPRECATED = ["exa-search", "context7", "jira", "agent-browser"];

function notice(plugin: string): string {
  return `toolu: ${plugin}/session-start: ${plugin} is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove ${plugin} --host opencode --yes`;
}

const noticeLines = (logged: readonly Logged[]) =>
  logged.filter((line) => line.message.includes(" is deprecated and will be removed in "));

test("each deprecated plugin logs one notice at load, none per tool call or compaction", async () => {
  using sb = createSandbox();
  mkdirSync(join(sb.project, ".opencode/toolu"), { recursive: true });
  writeFileSync(
    join(sb.project, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled: ["toolu", ...DEPRECATED] }),
  );
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(sb, logged, ""));
  try {
    const atLoad = noticeLines(logged);
    expect(atLoad.map((line) => line.message).toSorted()).toEqual(
      DEPRECATED.map(notice).toSorted(),
    );
    expect(atLoad.every((line) => line.level === "info")).toBe(true);

    expect(await refusal(hooks, "echo ok")).toBe("allowed");
    await bash(hooks, sb, "echo ok");
    const compaction = { context: ["keep-me"] };
    await hook(hooks, "experimental.session.compacting")({ sessionID: "ses_a" }, compaction);
    const system = await systemLines(hooks, "ses_a");
    expect(noticeLines(logged)).toHaveLength(DEPRECATED.length);

    const model = [...system, ...compaction.context].join("\n");
    expect(model).not.toContain("is deprecated");
  } finally {
    await hooks.dispose?.();
  }
});
