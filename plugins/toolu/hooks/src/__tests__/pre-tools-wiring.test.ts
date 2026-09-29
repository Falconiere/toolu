/**
 * AC-4 (#258), AC-5 (#260), AC-4 and AC-7 (#262): the edit/shell/search
 * PreToolUse entry runs the dispatcher bundle, the `mcp__` entry the
 * mcp-blocker bundle and the subagent entry the agent-tier bundle, each through
 * the generated launcher, which blocks when Bun is missing; the built-in table
 * keeps `mod.sh`'s byte order and every module is native, so the dispatcher
 * bundle carries no bash fallback; and a dispatcher crash blocks the tool
 * instead of allowing it.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { BUILTIN_MODULES, builtins, NATIVE_MODULES } from "../pre-tools/builtins.ts";

const PLUGIN = resolve(import.meta.dir, "../../..");
const LAUNCHER = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: "pre-tools" });
const MCP_LAUNCHER = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: "mcp-tools" });
const AGENT_LAUNCHER = launcherCommand({
  plugin: "toolu",
  event: "PreToolUse",
  entry: "agent-tier",
});

function withTempDir<T>(work: (dir: string) => T): T {
  const dir = mkdtempSync(join(tmpdir(), "toolu-pretools-"));
  try {
    return work(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("the built-in table keeps mod.sh's byte order; every module is native", () => {
  const sorted = [...BUILTIN_MODULES].toSorted((a, b) =>
    Buffer.compare(Buffer.from(a), Buffer.from(b)),
  );
  expect<string[]>([...BUILTIN_MODULES]).toEqual(sorted);
  const table = builtins(join(PLUGIN, "hooks"));
  expect(table.map((m) => m.name)).toEqual([...BUILTIN_MODULES]);
  expect(table.every((m) => m.kind === "native")).toBe(true);
  expect([...BUILTIN_MODULES]).toEqual(Object.keys(NATIVE_MODULES));
  expect(BUILTIN_MODULES).toHaveLength(9);
});

test("the dispatcher bundle has no built-in bash path and no bash bridge", () => {
  const bundle = readFileSync(join(PLUGIN, "hooks/dist/pre-tools.js"), "utf8");
  for (const needle of ["bashModule", "runPreToolBridge", "src/bridge/", "pre-tools/modules"]) {
    expect(bundle).not.toContain(needle);
  }
});

test("the ported modules' bats suites are gone from the index", () => {
  const listed = spawnSync("git", ["ls-files", "--", "hooks/pre-tools/**/*.bats"], {
    cwd: PLUGIN,
    encoding: "utf8",
  });
  expect(listed.status).toBe(0);
  expect(listed.stdout).toBe("");
});

test("hooks.json runs every PreToolUse bundle through the launcher", () => {
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
          hooks: [expect.objectContaining({ type: "command", command: MCP_LAUNCHER })],
        },
        {
          matcher: "spawn_agent|Agent|Task",
          hooks: [expect.objectContaining({ type: "command", command: AGENT_LAUNCHER })],
        },
      ],
    },
  });
});

for (const [entry, command, input] of [
  ["pre-tools", LAUNCHER, '{"tool_name":"Bash","tool_input":{"command":"ls"}}'],
  ["mcp-tools", MCP_LAUNCHER, '{"tool_name":"mcp__exampleblocked__search","tool_input":{}}'],
  ["agent-tier", AGENT_LAUNCHER, '{"tool_name":"Agent","tool_input":{"model":"opus"}}'],
] as const) {
  test.concurrent(`the ${entry} launcher blocks with exit 2 when Bun is missing`, () => {
    withTempDir((home) => {
      const result = spawnSync("/bin/sh", ["-c", command], {
        env: { PATH: "/usr/bin:/bin", HOME: home, CLAUDE_PLUGIN_ROOT: PLUGIN },
        input,
        encoding: "utf8",
      });
      expect(result.status).toBe(2);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("blocked: toolu plugin: Bun runtime not found");
    });
  });
}

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
