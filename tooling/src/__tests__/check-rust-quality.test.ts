/**
 * `tooling/src/check-rust-quality.ts` (#455) on real trees: this repository's
 * crates pass, and a 320-code-line file under the repository's 300-line limit
 * fails with the rust-quality plugin's own message.
 */
import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { checkTree, rustFiles } from "../check-rust-quality.ts";

const REPO = resolve(import.meta.dir, "../../..");

function tree(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), "rust-quality-"));
  for (const [rel, body] of Object.entries(files)) {
    mkdirSync(join(root, rel, ".."), { recursive: true });
    writeFileSync(join(root, rel), body);
  }
  const init = spawnSync("git", ["-C", root, "init", "-q"], { encoding: "utf8" });
  if (init.status !== 0) throw new Error(init.stderr);
  return root;
}

/** A module of `n` code lines after a doc line and a blank line. */
function module(n: number): string {
  const lines = [...Array.from({ length: n }).keys()].map(
    (i) => `pub const C${String(i)}: u8 = 1;`,
  );
  return `//! demo\n\n${lines.join("\n")}\n`;
}

describe("check-rust-quality", () => {
  test("this repository's crates pass at its own thresholds", () => {
    expect(rustFiles(REPO).length).toBeGreaterThan(20);
    expect(checkTree(REPO)).toEqual([]);
  });

  test("a 320-code-line file fails at the repository's 300-line limit, 300 passes", () => {
    const config = readFileSync(join(REPO, ".claude/toolu.config.json"), "utf8");
    const root = tree({
      ".claude/toolu.config.json": config,
      "crates/demo/src/over.rs": module(320),
      "crates/demo/src/at.rs": module(300),
      "tools/outside.rs": module(400),
    });
    try {
      const errors = checkTree(root);
      expect(errors).toHaveLength(1);
      expect(errors[0]).toContain(
        "File exceeds 300-line limit: crates/demo/src/over.rs (320 code lines",
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("the script fails closed (exit 2) when ast-grep is not on PATH", () => {
    const res = spawnSync(process.execPath, [join(REPO, "tooling/src/check-rust-quality.ts")], {
      encoding: "utf8",
      env: { ...process.env, PATH: `${dirname(process.execPath)}:/usr/bin:/bin` },
    });
    expect(res.status).toBe(2);
    expect(res.stderr).toBe(
      "rust-quality: ast-grep is not installed (npm install -g @ast-grep/cli)\n",
    );
  });

  test("without the repository config the plugin default (500) applies", () => {
    const root = tree({ "crates/demo/src/over.rs": module(320) });
    try {
      expect(checkTree(root)).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
