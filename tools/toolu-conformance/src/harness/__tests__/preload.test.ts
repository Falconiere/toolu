import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { createSandbox } from "../sandbox.ts";
import { run } from "../spawn.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");

test.concurrent("a suite run from the repo root outlives bun's 5 s default timeout", async () => {
  using sb = createSandbox();
  const slow = sb.write(
    "slow.test.ts",
    'import { test } from "bun:test";\ntest("slow", async () => { await Bun.sleep(5_500); });\n',
  );
  const res = await run([process.execPath, "test", slow], { cwd: ROOT });
  expect(res.stdout + res.stderr).toContain("1 pass");
  expect(res.exitCode).toBe(0);
});
