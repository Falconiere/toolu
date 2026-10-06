/** Rust-generated lifecycle launchers through OpenCode's real context subprocess path. */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Part, UserMessage } from "@opencode-ai/sdk";
import { fixturePlugin, tempRoot } from "../../bootstrap/__tests__/fixtures.ts";
import { contextJobs, createContextHooks } from "../context-delivery.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
const SESSION_COMMAND = readFileSync(
  join(ROOT, "crates/core/protocol/src/tests/fixtures/launcher-session-start.txt"),
  "utf8",
);
const PROMPT_HOOK: unknown = JSON.parse(
  readFileSync(join(ROOT, "tooling/fixtures/native-launcher/user-prompt-submit.json"), "utf8"),
);
const COMPACT_HOOK: unknown = JSON.parse(
  readFileSync(join(ROOT, "tooling/fixtures/native-launcher/pre-compact.json"), "utf8"),
);

function executable(root: string): string {
  const path = join(root, "native-bin");
  writeFileSync(
    path,
    `#!/bin/sh
if [ "$1" = "--hook-protocol" ]; then printf '1\\n'; exit 0; fi
case "$2" in
  session-start) printf '%s\\n' '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"native compact context"}}' ;;
  user-prompt-submit) printf '%s\\n' '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"native prompt context"}}' ;;
  pre-compact) printf '%s\\n' '{"hookSpecificOutput":{"hookEventName":"PreCompact","additionalContext":"native precompact context"}}' ;;
  *) exit 3 ;;
esac
`,
  );
  chmodSync(path, 0o755);
  return path;
}

function nativeHooks(root: string, pluginDir: string) {
  const jobs = contextJobs([{ name: "toolu", pluginDir }]);
  if (!jobs.ok) throw new Error(jobs.reason);
  const projectRoot = join(root, "project");
  mkdirSync(projectRoot);
  return createContextHooks(
    {
      startupLines: [],
      notices: [],
      prompt: jobs.prompt,
      compact: jobs.compact,
      bun: process.execPath,
      projectRoot,
      env: { TOOLU_BIN: executable(root), TOOLU_BUN: process.execPath },
    },
    () => Promise.resolve(),
  );
}

function message(): UserMessage {
  return {
    id: "msg_native",
    sessionID: "ses_native",
    role: "user",
    time: { created: 0 },
    agent: "build",
    model: { providerID: "probe", modelID: "scripted" },
  };
}

test("a generated native compact SessionStart runs through the real subprocess", async () => {
  using root = tempRoot("toolu-native-compact-");
  const plugin = fixturePlugin(root.path, "toolu", {
    entries: { "session-start": "process.exit(0);\n" },
  });
  writeFileSync(
    join(plugin.pluginDir, "hooks", "hooks.json"),
    JSON.stringify({
      hooks: {
        SessionStart: [
          {
            matcher: "compact",
            hooks: [{ type: "command", command: SESSION_COMMAND, timeout: 60 }],
          },
        ],
      },
    }),
  );
  const hooks = nativeHooks(root.path, plugin.pluginDir);
  const output = { context: ["keep-me"] };
  await hooks.compacting({ sessionID: "ses_native" }, output);
  expect(output.context).toContain("native compact context");
  expect(output.context).toContain("keep-me");
  await hooks.dispose();
});

test("generated native prompt and PreCompact hooks deliver their context", async () => {
  using root = tempRoot("toolu-native-context-events-");
  const plugin = fixturePlugin(root.path, "toolu", {
    entries: { "user-prompt-submit": "", "pre-compact": "" },
  });
  writeFileSync(
    join(plugin.pluginDir, "hooks", "hooks.json"),
    JSON.stringify({
      hooks: {
        UserPromptSubmit: [{ matcher: "prompt", hooks: [PROMPT_HOOK] }],
        PreCompact: [{ matcher: "auto", hooks: [COMPACT_HOOK] }],
      },
    }),
  );
  const hooks = nativeHooks(root.path, plugin.pluginDir);
  const parts: Part[] = [
    { id: "prt_user", sessionID: "ses_native", messageID: "msg_native", type: "text", text: "go" },
  ];
  const output = { message: message(), parts };
  await hooks.prompt({ sessionID: "ses_native", messageID: "msg_native" }, output);
  expect(
    output.parts.some((part) => part.type === "text" && part.text === "native prompt context"),
  ).toBe(true);
  const compact = { context: ["keep-me"] };
  await hooks.compacting({ sessionID: "ses_native" }, compact);
  expect(compact.context).toContain("native precompact context");
  expect(compact.context).toContain("keep-me");
  await hooks.dispose();
});
