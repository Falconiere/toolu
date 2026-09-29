import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";

// The live smoke needs herdr and a real Claude fixer, so it runs by hand; this
// pins its fail-fast contract when a required tool is absent.

const ROOT = resolve(import.meta.dir, "../../..");

test.concurrent("the herdr smoke fails fast naming the missing tool", async () => {
  const res = await run(
    [process.execPath, resolve(ROOT, "tooling/src/pr-babysit-herdr-smoke.ts")],
    {
      cwd: ROOT,
      env: { PATH: "/usr/bin:/bin" },
    },
  );
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("pr-babysit-herdr-smoke: FAIL: herdr is required");
});
