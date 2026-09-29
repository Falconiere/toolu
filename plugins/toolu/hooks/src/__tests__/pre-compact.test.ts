/**
 * toolu's PreCompact bundle (#263, ported from pre-compact.bats): silent and
 * exit 0 whatever arrives on stdin, and it reads all of it so the host never
 * sees a broken pipe.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { launcherHook } from "@toolu/core/launcher";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { hookCommand } from "@toolu/conformance/harness/startup";
import { PLUGIN } from "./lifecycle-cases.ts";

const COMMAND = hookCommand(PLUGIN, "PreCompact", "pre-compact");

function env(home: string, extra: EnvPatch = {}): EnvPatch {
  return {
    PATH: "/usr/bin:/bin",
    HOME: home,
    TOOLU_BUN: process.execPath,
    CLAUDE_PLUGIN_ROOT: PLUGIN,
    ...extra,
  };
}

test("hooks.json runs the launcher on auto compaction", async () => {
  const hooks: unknown = await Bun.file(join(PLUGIN, "hooks", "hooks.json")).json();
  expect(hooks).toMatchObject({
    hooks: {
      PreCompact: [
        {
          matcher: "auto",
          hooks: [launcherHook({ plugin: "toolu", event: "PreCompact", entry: "pre-compact" })],
        },
      ],
    },
  });
});

test("silent for Codex auto-compaction input", async () => {
  using sb = createSandbox();
  const res = await run(["sh", "-c", COMMAND], {
    cwd: sb.project,
    env: env(sb.home, { TOOLU_HOST_OVERRIDE: "codex", CODEX_HOME: sb.codexHome }),
    stdin: '{"hook_event_name":"PreCompact","trigger":"auto"}',
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});

test("drains 1 MiB of stdin without closing the producer pipe", async () => {
  using sb = createSandbox();
  const script = `set -o pipefail; head -c 1048576 /dev/zero | { ${COMMAND}; }`;
  const res = await run(["bash", "-c", script], { cwd: sb.project, env: env(sb.home) });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});

test("silent with empty stdin", async () => {
  using sb = createSandbox();
  const res = await run(["sh", "-c", `${COMMAND} < /dev/null`], {
    cwd: sb.project,
    env: env(sb.home),
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});

test("silent when disabled by config", async () => {
  using sb = createSandbox();
  const config = join(sb.root, "config");
  mkdirSync(config, { recursive: true });
  writeFileSync(join(config, "toolu.config.json"), '{"hooks":{"pre-compact":false}}\n');
  const res = await run(["sh", "-c", COMMAND], {
    cwd: sb.project,
    env: env(sb.home, { TOOLU_CONFIG_DIR: config, TOOLU_PROJECT_DIR: sb.project }),
    stdin: '{"hook_event_name":"PreCompact","trigger":"auto"}',
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});
