/**
 * `diffSha` vs bash `toolu_diff_sha` (#255) on a real repo: a feature branch,
 * an empty diff (the empty-blob id) and a bad base ref (no hash; bash exits
 * non-zero).
 */
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { diffSha } from "../diff-sha.ts";

const DIFF_SHA_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/diff-sha.sh");
const EMPTY_BLOB = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

async function bashDiffSha(root: string, base: string): Promise<{ code: number; sha: string }> {
  const res = await run([
    "bash",
    "-c",
    '. "$1"; toolu_diff_sha "$2" "$3"',
    "_",
    DIFF_SHA_SH,
    root,
    base,
  ]);
  return { code: res.exitCode, sha: res.stdout.trim() };
}

test("a feature branch hashes identically in bash and TypeScript", async () => {
  using sb = createSandbox({ git: true, files: { "a.txt": "one\n" } });
  sb.git("checkout", "-q", "-b", "feat/x");
  sb.write("a.txt", "one\ntwo \u007f é\n");
  sb.write("b.bin", "\u0000\u0001binary");
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "change");
  const bash = await bashDiffSha(sb.project, "main");
  expect(bash.code).toBe(0);
  expect(diffSha(sb.project, "main")).toBe(bash.sha);
  expect(bash.sha).not.toBe(EMPTY_BLOB);
});

test("a large diff (over node's 1 MiB spawnSync buffer) still hashes identically", async () => {
  using sb = createSandbox({ git: true });
  sb.git("checkout", "-q", "-b", "feat/big");
  sb.write("big.txt", "line of text\n".repeat(200_000));
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "big");
  const bash = await bashDiffSha(sb.project, "main");
  expect(diffSha(sb.project, "main")).toBe(bash.sha);
});

test("an empty diff returns the empty-blob id in both", async () => {
  using sb = createSandbox({ git: true });
  sb.git("checkout", "-q", "-b", "feat/empty");
  expect(diffSha(sb.project, "main")).toBe(EMPTY_BLOB);
  expect(await bashDiffSha(sb.project, "main")).toEqual({ code: 0, sha: EMPTY_BLOB });
});

test("a bad base ref gives no hash; bash exits non-zero with empty stdout", async () => {
  using sb = createSandbox({ git: true });
  expect(diffSha(sb.project, "no-such-ref")).toBeUndefined();
  const bash = await bashDiffSha(sb.project, "no-such-ref");
  expect(bash.code).not.toBe(0);
  expect(bash.sha).toBe("");
});

test("a directory that is not a repository gives no hash", () => {
  using sb = createSandbox();
  expect(diffSha(sb.project, "main")).toBeUndefined();
});

test("a dash-prefixed base ref is a revision, never a git option", () => {
  using sb = createSandbox({ git: true });
  const planted = join(sb.root, "planted.txt");
  expect(diffSha(sb.project, `--output=${planted}`)).toBeUndefined();
  expect(existsSync(planted)).toBe(false);
});
