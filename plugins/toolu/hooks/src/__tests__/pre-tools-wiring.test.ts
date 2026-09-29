/**
 * AC-4 (#258): the three PreToolUse hooks.json entries keep their matchers and
 * run their bundles through the generated launcher, which blocks when Bun is
 * missing; the built-in table lists exactly the bash modules `mod.sh` globs.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { BUILTIN_MODULES } from "../pre-tools/builtins.ts";

const PLUGIN = resolve(import.meta.dir, "../../..");

const ENTRIES = [
  { matcher: "apply_patch|Edit|Write|MultiEdit|Bash|Shell|Grep", entry: "pre-tools" },
  { matcher: "mcp__", entry: "pre-tools-mcp" },
  { matcher: "spawn_agent|Agent|Task", entry: "pre-tools-agent" },
] as const;

function preToolUse(): unknown {
  const hooks: unknown = JSON.parse(readFileSync(join(PLUGIN, "hooks/hooks.json"), "utf8"));
  return hooks;
}

test("the built-in table is modules/*.sh in the byte order mod.sh globs", () => {
  const onDisk = readdirSync(join(PLUGIN, "hooks/pre-tools/modules"))
    .filter((file) => file.endsWith(".sh"))
    .toSorted((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))
    .map((file) => file.slice(0, -".sh".length));
  expect<string[]>([...BUILTIN_MODULES]).toEqual(onDisk);
});

test("every PreToolUse entry keeps its matcher and runs its bundle through the launcher", () => {
  expect(preToolUse()).toMatchObject({
    hooks: {
      PreToolUse: ENTRIES.map(({ matcher, entry }) => ({
        matcher,
        hooks: [
          expect.objectContaining({
            type: "command",
            command: launcherCommand({ plugin: "toolu", event: "PreToolUse", entry }),
          }),
        ],
      })),
    },
  });
});

test.concurrent.each(ENTRIES.map((e) => e.entry))(
  "%s blocks with exit 2 when Bun is missing",
  (entry) => {
    const home = mkdtempSync(join(tmpdir(), "toolu-nobun-"));
    try {
      const result = spawnSync(
        "/bin/sh",
        ["-c", launcherCommand({ plugin: "toolu", event: "PreToolUse", entry })],
        {
          env: { PATH: "/usr/bin:/bin", HOME: home, CLAUDE_PLUGIN_ROOT: PLUGIN },
          input: '{"tool_name":"Bash","tool_input":{"command":"ls"}}',
          encoding: "utf8",
        },
      );
      expect(result.status).toBe(2);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("blocked: toolu plugin: Bun runtime not found");
    } finally {
      rmSync(home, { recursive: true, force: true });
    }
  },
);
