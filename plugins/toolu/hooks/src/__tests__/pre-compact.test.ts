/**
 * toolu's PreCompact bundle (#263, ported from pre-compact.bats): silent and
 * exit 0 whatever arrives on stdin, and it reads all of it so the host never
 * sees a broken pipe.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { launcherHook } from "@toolu/core/launcher";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { PLUGIN } from "./lifecycle-cases.ts";

function preCompactArgv(): string[] {
  return entryArgv("toolu", "pre-compact", PLUGIN);
}

function shellQuote(word: string): string {
  return `'${word.replaceAll("'", "'\\''")}'`;
}

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
  const res = await run(preCompactArgv(), {
    cwd: sb.project,
    env: env(sb.home, { TOOLU_HOST_OVERRIDE: "codex", CODEX_HOME: sb.codexHome }),
    stdin: '{"hook_event_name":"PreCompact","trigger":"auto"}',
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});

test("drains 1 MiB of stdin without closing the producer pipe", async () => {
  using sb = createSandbox();
  const command = preCompactArgv().map(shellQuote).join(" ");
  const script = `set -o pipefail; head -c 1048576 /dev/zero | { ${command}; }`;
  const res = await run(["bash", "-c", script], { cwd: sb.project, env: env(sb.home) });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});

test("silent with empty stdin", async () => {
  using sb = createSandbox();
  const res = await run(preCompactArgv(), {
    cwd: sb.project,
    env: env(sb.home),
    stdin: "",
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});

test("stays silent with the hook disabled in config", async () => {
  using sb = createSandbox();
  const config = join(sb.root, "config");
  mkdirSync(config, { recursive: true });
  writeFileSync(join(config, "toolu.config.json"), '{"hooks":{"pre-compact":false}}\n');
  const res = await run(preCompactArgv(), {
    cwd: sb.project,
    env: env(sb.home, { TOOLU_CONFIG_DIR: config, TOOLU_PROJECT_DIR: sb.project }),
    stdin: '{"hook_event_name":"PreCompact","trigger":"auto"}',
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});
