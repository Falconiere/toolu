/**
 * epic-orchestrator's Codex dependency check through its hooks.json launcher
 * (#269, ported from dependency.bats): a `codex` on PATH prints real
 * `plugin list --json` shapes.
 */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { runStartupHook } from "@toolu/conformance/harness/startup";

const PLUGIN = resolve(import.meta.dir, "../../..");
const REQUIRED = [
  "toolu@toolu",
  "delivery-flow@toolu",
  "toolu-review@toolu",
  "pr-babysit@toolu",
  "brainstorm@toolu",
];

function codexBin(sb: Sandbox, stdout: string): string {
  const bin = join(sb.root, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "codex"), `#!/bin/sh\ncat <<'JSON'\n${stdout}\nJSON\n`);
  chmodSync(join(bin, "codex"), 0o755);
  return bin;
}

function hook(sb: Sandbox, listing: object, codexHost: boolean) {
  const env = {
    HOME: sb.home,
    PATH: `${codexBin(sb, JSON.stringify(listing))}:${process.env["PATH"] ?? ""}`,
    CLAUDE_PLUGIN_ROOT: PLUGIN,
    ...(codexHost ? { PLUGIN_ROOT: PLUGIN } : {}),
  };
  return runStartupHook(PLUGIN, "check-deps", sb, env);
}

test.concurrent("warns with every missing plugin and its Codex install command", async () => {
  using sb = createSandbox();
  const res = await hook(sb, { installed: [] }, true);
  const context = REQUIRED.map((id) => ` ${id} (install with: codex plugin add ${id})`).join("");
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toBe(
    `${JSON.stringify(
      {
        hookSpecificOutput: {
          hookEventName: "SessionStart",
          additionalContext: `WARN: this plugin requires${context}`,
        },
      },
      null,
      2,
    )}\n`,
  );
});

test.concurrent("names only what is missing or disabled", async () => {
  using sb = createSandbox();
  const installed = REQUIRED.map((pluginId) => ({ pluginId, installed: true, enabled: true }));
  installed[1] = { pluginId: "delivery-flow@toolu", installed: true, enabled: false };
  const res = await hook(sb, { installed: installed.slice(0, 4) }, true);
  expect(res.stdout).toContain(
    'WARN: this plugin requires delivery-flow@toolu (install with: codex plugin add delivery-flow@toolu) brainstorm@toolu (install with: codex plugin add brainstorm@toolu)"',
  );
});

test.concurrent("silent when every dependency is installed and enabled", async () => {
  using sb = createSandbox();
  const installed = REQUIRED.map((pluginId) => ({ pluginId, installed: true, enabled: true }));
  const res = await hook(sb, { installed }, true);
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});

test.concurrent("silent on Claude Code whatever codex would list", async () => {
  using sb = createSandbox();
  const res = await hook(sb, { installed: [] }, false);
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});
