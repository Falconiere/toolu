/** The #412 generated shell command through the real bootstrap subprocess path. */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { bootstrapRuntime } from "../runtime.ts";
import { fixturePlugin, REPO_ROOT, tempRoot } from "./fixtures.ts";

const NATIVE_COMMAND = readFileSync(
  resolve(
    import.meta.dir,
    "../../../../../crates/core/protocol/src/tests/fixtures/launcher-session-start.txt",
  ),
  "utf8",
);

function nativePlugin(root: string) {
  const plugin = fixturePlugin(root, "toolu", {
    entries: { "session-start": "process.exit(0);\n" },
  });
  writeFileSync(
    join(plugin.pluginDir, "hooks", "hooks.json"),
    JSON.stringify({
      hooks: {
        SessionStart: [
          {
            matcher: "startup",
            hooks: [
              {
                type: "command",
                command: NATIVE_COMMAND,
                timeout: 60,
              },
            ],
          },
        ],
      },
    }),
  );
  return plugin;
}

function project(root: string): string {
  const dir = join(root, "project");
  mkdirSync(dir);
  return dir;
}

test("a generated native SessionStart command gives OpenCode its context", async () => {
  using root = tempRoot("toolu-native-context-");
  const plugin = nativePlugin(root.path);
  const executable = join(root.path, "native-bin");
  writeFileSync(
    executable,
    `#!/bin/sh
if [ "$1" = "--hook-protocol" ]; then printf '1\\n'; exit 0; fi
if [ "$1" = "hook" ] && [ "$2" = "session-start" ]; then
  printf '%s\\n' '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"native context delivered"}}'
  exit 0
fi
exit 3
`,
  );
  chmodSync(executable, 0o755);
  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: project(root.path),
    dataRoot: join(root.path, "data"),
    plugins: [plugin],
    env: { TOOLU_BIN: executable, TOOLU_BUN: process.execPath },
  });
  expect(result.status).toBe("ready");
  if (result.status !== "ready") throw new Error(result.reason);
  expect(result.plugins[0]?.entries[0]?.additionalContext).toBe("native context delivered");
});

test("a missing TOOLU_BIN reports both install commands as startup context", async () => {
  using root = tempRoot("toolu-native-missing-");
  const plugin = nativePlugin(root.path);
  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: project(root.path),
    dataRoot: join(root.path, "data"),
    plugins: [plugin],
    env: { TOOLU_BIN: join(root.path, "absent"), TOOLU_BUN: process.execPath },
  });
  expect(result.status).toBe("ready");
  if (result.status !== "ready") throw new Error(result.reason);
  const message = result.plugins[0]?.entries[0]?.systemMessage ?? "";
  expect(message).toContain("curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash");
  expect(message).toContain("brew install falconiere/tap/toolu");
});

test("the native launcher's Bun fallback does not load the project .env", async () => {
  using root = tempRoot("toolu-native-fallback-env-");
  const plugin = nativePlugin(root.path);
  const projectRoot = project(root.path);
  writeFileSync(join(projectRoot, ".env"), "TOOLU_NATIVE_FALLBACK_SECRET=from-project\n");
  writeFileSync(
    join(plugin.pluginDir, "hooks", "dist", "session-start.js"),
    'console.log(JSON.stringify({hookSpecificOutput:{hookEventName:"SessionStart",additionalContext:process.env.TOOLU_NATIVE_FALLBACK_SECRET ?? "absent"}}));\n',
  );
  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot,
    dataRoot: join(root.path, "data"),
    plugins: [plugin],
    env: { TOOLU_BIN: "", TOOLU_BUN: process.execPath, HOME: root.path, PATH: "/usr/bin:/bin" },
  });
  expect(result.status).toBe("ready");
  if (result.status !== "ready") throw new Error(result.reason);
  expect(result.plugins[0]?.entries[0]?.additionalContext).toBe("absent");
  expect(result.diagnostics.some((line) => line.includes("running the Bun bundle"))).toBe(true);
});
