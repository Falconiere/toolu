/**
 * `toolu plugins` against the real Claude Code CLI in a temporary
 * `CLAUDE_CONFIG_DIR` (ported from the TypeScript installer's install test):
 * install a standalone plugin, see it already installed, list it, remove it.
 * It needs `claude` on PATH and the network; without `claude` it is skipped.
 */
import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { requiredBuiltTooluBinary } from "@toolu/conformance/harness/entry-command";

const hasClaude = Bun.which("claude") !== null;
const configDir = mkdtempSync(join(tmpdir(), "toolu-claude-"));
const env = { ...process.env, CLAUDE_CONFIG_DIR: configDir };
afterAll(() => rmSync(configDir, { recursive: true, force: true }));

function plugins(args: readonly string[]) {
  const res = spawnSync(requiredBuiltTooluBinary(), ["plugins", ...args, "--host", "claude"], {
    env,
    encoding: "utf8",
    timeout: 300_000,
  });
  return { code: res.status, stdout: res.stdout, stderr: res.stderr };
}

describe.skipIf(!hasClaude)("the real claude CLI", () => {
  test("installs a standalone plugin, reports it already installed, lists and removes it", () => {
    const first = plugins(["install", "ast-grep"]);
    expect(first.stderr).toBe("");
    expect(first.code).toBe(0);
    expect(first.stdout).toContain("+ ast-grep");
    const second = plugins(["install", "ast-grep"]);
    expect(second.stdout).toMatch(/= ast-grep +already installed at \d/);
    const listed = plugins(["list", "--json"]);
    const rows: { name: string; installed: boolean }[] = JSON.parse(listed.stdout);
    expect(rows.find((row) => row.name === "ast-grep")?.installed).toBe(true);
    expect(rows.find((row) => row.name === "rust-quality")?.installed).toBe(false);
    const removed = plugins(["remove", "ast-grep", "--yes"]);
    expect(removed.code).toBe(0);
    expect(removed.stdout).toContain("- ast-grep");
  }, 600_000);
});
