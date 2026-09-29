/**
 * AC-4 (#258): the edit/shell/search PreToolUse entry runs the dispatcher
 * bundle through the generated launcher, which blocks when Bun is missing; the
 * MCP and subagent entries keep their bash scripts until #260/#262 port them;
 * the built-in table keeps `mod.sh`'s byte order, ported modules (#261) are
 * native and every other one runs its script; and a
 * dispatcher crash blocks the tool instead of allowing it.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { BUILTIN_MODULES, builtins, NATIVE_MODULES } from "../pre-tools/builtins.ts";

const PLUGIN = resolve(import.meta.dir, "../../..");
const LAUNCHER = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: "pre-tools" });

function withTempDir<T>(work: (dir: string) => T): T {
  const dir = mkdtempSync(join(tmpdir(), "toolu-pretools-"));
  try {
    return work(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("the built-in table keeps mod.sh's byte order; ported modules are native", () => {
  const sorted = [...BUILTIN_MODULES].toSorted((a, b) =>
    Buffer.compare(Buffer.from(a), Buffer.from(b)),
  );
  expect<string[]>([...BUILTIN_MODULES]).toEqual(sorted);
  const table = builtins(join(PLUGIN, "hooks"));
  expect(table.map((m) => m.name.replace(/\.sh$/, ""))).toEqual([...BUILTIN_MODULES]);
  for (const module of table) {
    const native = Object.hasOwn(NATIVE_MODULES, module.name);
    expect(module.kind).toBe(native ? "native" : "bash");
    if (module.kind === "bash")
      expect(readdirSync(join(PLUGIN, "hooks/pre-tools/modules"))).toContain(module.name);
  }
  expect(Object.keys(NATIVE_MODULES).toSorted()).toEqual([
    "bash-commands",
    "commit-gate",
    "quality-gate",
  ]);
});

test("hooks.json runs the dispatcher bundle through the launcher and leaves the standalone scripts", () => {
  const hooks: unknown = JSON.parse(readFileSync(join(PLUGIN, "hooks/hooks.json"), "utf8"));
  expect(hooks).toMatchObject({
    hooks: {
      PreToolUse: [
        {
          matcher: "apply_patch|Edit|Write|MultiEdit|Bash|Shell|Grep",
          hooks: [expect.objectContaining({ type: "command", command: LAUNCHER })],
        },
        {
          matcher: "mcp__",
          hooks: [
            {
              type: "command",
              command: "${CLAUDE_PLUGIN_ROOT}/hooks/pre-tools/modules/mcp-blocker.sh",
            },
          ],
        },
        {
          matcher: "spawn_agent|Agent|Task",
          hooks: [
            { type: "command", command: "${CLAUDE_PLUGIN_ROOT}/hooks/pre-tools/agent-tier.sh" },
          ],
        },
      ],
    },
  });
});

test.concurrent("the launcher blocks with exit 2 when Bun is missing", () => {
  withTempDir((home) => {
    const result = spawnSync("/bin/sh", ["-c", LAUNCHER], {
      env: { PATH: "/usr/bin:/bin", HOME: home, CLAUDE_PLUGIN_ROOT: PLUGIN },
      input: '{"tool_name":"Bash","tool_input":{"command":"ls"}}',
      encoding: "utf8",
    });
    expect(result.status).toBe(2);
    expect(result.stdout).toBe("");
    expect(result.stderr).toContain("blocked: toolu plugin: Bun runtime not found");
  });
});

test.concurrent("a dispatcher that throws blocks the tool call with exit 2", () => {
  withTempDir((dir) => {
    const entry = join(dir, "crash.ts");
    const hookMain = join(PLUGIN, "hooks/src/pre-tools/hook-main.ts");
    writeFileSync(
      entry,
      `import { hookMain } from ${JSON.stringify(hookMain)};\n` +
        `await hookMain(import.meta.dir, () => { throw new Error("walk exploded"); });\n`,
    );
    const result = spawnSync(process.execPath, [entry], { input: "{}", encoding: "utf8" });
    expect(result.status).toBe(2);
    expect(result.stdout).toBe("");
    expect(result.stderr).toBe("blocked: toolu PreToolUse dispatcher failed: walk exploded\n");
  });
});
