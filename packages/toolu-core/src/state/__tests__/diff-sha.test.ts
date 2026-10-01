/** Content-addressed diff hashes on real repositories. */
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { diffSha } from "../diff-sha.ts";

const EMPTY_BLOB = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

test("a feature branch has a nonempty content hash", () => {
  using sb = createSandbox({ git: true, files: { "a.txt": "one\n" } });
  sb.git("checkout", "-q", "-b", "feat/x");
  sb.write("a.txt", "one\ntwo \u007f é\n");
  sb.write("b.bin", "\u0000\u0001binary");
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "change");
  const sha = diffSha(sb.project, "main");
  expect(sha).toMatch(/^[0-9a-f]{40,64}$/);
  expect(sha).not.toBe(EMPTY_BLOB);
});

test("a large diff (over 1 MiB) still hashes", () => {
  using sb = createSandbox({ git: true });
  sb.git("checkout", "-q", "-b", "feat/big");
  sb.write("big.txt", "line of text\n".repeat(200_000));
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "big");
  expect(diffSha(sb.project, "main")).toMatch(/^[0-9a-f]{40,64}$/);
});

test("an empty diff returns the empty-blob id", () => {
  using sb = createSandbox({ git: true });
  sb.git("checkout", "-q", "-b", "feat/empty");
  expect(diffSha(sb.project, "main")).toBe(EMPTY_BLOB);
});

test("a bad base ref gives no hash", () => {
  using sb = createSandbox({ git: true });
  expect(diffSha(sb.project, "no-such-ref")).toBeUndefined();
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
