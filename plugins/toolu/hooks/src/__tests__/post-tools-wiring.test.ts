/**
 * AC-6 (#259): the edit/shell/search PostToolUse entry runs the dispatcher
 * bundle through the generated launcher, which advises and exits 0 when Bun is
 * missing (PostToolUse is not an enforcing event); the built-in table names
 * exactly the bash modules `mod.sh` globs; and a dispatcher crash exits 2 with
 * one stderr line instead of passing silently.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { BUILTIN_MODULES, builtins } from "../post-tools/builtins.ts";

const PLUGIN = resolve(import.meta.dir, "../../..");
const LAUNCHER = launcherCommand({ plugin: "toolu", event: "PostToolUse", entry: "post-tools" });

function withTempDir<T>(work: (dir: string) => T): T {
  const dir = mkdtempSync(join(tmpdir(), "toolu-posttools-"));
  try {
    return work(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("the built-in table is modules/*.sh in the byte order mod.sh globs", () => {
  const onDisk = readdirSync(join(PLUGIN, "hooks/post-tools/modules"))
    .filter((file) => file.endsWith(".sh"))
    .toSorted((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))
    .map((file) => file.slice(0, -".sh".length));
  expect<string[]>([...BUILTIN_MODULES]).toEqual(onDisk);
  expect(builtins().map((m) => m.name)).toEqual(onDisk.map((name) => `${name}.sh`));
  expect(builtins().every((m) => m.kind === "native")).toBe(true);
});

test("hooks.json runs the post dispatcher bundle through the launcher", () => {
  const hooks: unknown = JSON.parse(readFileSync(join(PLUGIN, "hooks/hooks.json"), "utf8"));
  expect(hooks).toMatchObject({
    hooks: {
      PostToolUse: [
        {
          matcher: "apply_patch|Edit|Write|MultiEdit|Bash|Shell|Grep",
          hooks: [expect.objectContaining({ type: "command", command: LAUNCHER })],
        },
      ],
    },
  });
});

test.concurrent("the launcher advises and exits 0 when Bun is missing", () => {
  withTempDir((home) => {
    const result = spawnSync("/bin/sh", ["-c", LAUNCHER], {
      env: { PATH: "/usr/bin:/bin", HOME: home, CLAUDE_PLUGIN_ROOT: PLUGIN },
      input: '{"tool_name":"Bash","tool_input":{"command":"bun test"}}',
      encoding: "utf8",
    });
    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({
      systemMessage: expect.stringContaining("toolu plugin: Bun runtime not found"),
    });
  });
});

test.concurrent("a post dispatcher that throws exits 2 with one stderr line", () => {
  withTempDir((dir) => {
    const entry = join(dir, "crash.ts");
    const hookMain = join(PLUGIN, "hooks/src/pre-tools/hook-main.ts");
    writeFileSync(
      entry,
      `import { hookMain } from ${JSON.stringify(hookMain)};\n` +
        `await hookMain(import.meta.dir, () => { throw new Error("walk exploded"); }, "PostToolUse");\n`,
    );
    const result = spawnSync(process.execPath, [entry], { input: "{}", encoding: "utf8" });
    expect(result.status).toBe(2);
    expect(result.stdout).toBe("");
    expect(result.stderr).toBe("toolu PostToolUse dispatcher failed: walk exploded\n");
  });
});

test.concurrent("the committed bundle runs a failing quality command end to end", () => {
  withTempDir((dir) => {
    spawnSync("git", ["init", "-q", dir]);
    const result = spawnSync(process.execPath, [join(PLUGIN, "hooks/dist/post-tools.js")], {
      cwd: dir,
      env: { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: dir, TOOLU_HOST_OVERRIDE: "claude" },
      input: JSON.stringify({
        tool_name: "Bash",
        tool_input: { command: "bun test" },
        tool_response: { exit_code: 1 },
      }),
      encoding: "utf8",
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Global quality gate failing");
    const gate: unknown = JSON.parse(
      readFileSync(join(dir, ".claude/tmp/quality-gate-status.json"), "utf8"),
    );
    expect(gate).toMatchObject({ status: "failing", source: "gate-status-hook" });
  });
});
