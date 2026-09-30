import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { expect, test } from "bun:test";
import { REPO_ROOT } from "./golden-harness.ts";

const capture = join(import.meta.dir, "golden-capture.ts");

for (const args of [["--base"], ["--base", ""]]) {
  test(`capture rejects ${JSON.stringify(args)}`, () => {
    const result = spawnSync("bun", ["run", capture, ...args], {
      cwd: REPO_ROOT,
      encoding: "utf8",
    });

    expect(result.status).toBe(2);
    expect(result.stderr).toContain("--base requires a non-empty commit");
    expect(result.stderr).toContain(
      "Usage: bun run plugins/ts-quality/hooks/src/__tests__/golden-capture.ts [--base <sha>]",
    );
    expect(result.stderr).not.toContain("git rev-parse");
  });
}
