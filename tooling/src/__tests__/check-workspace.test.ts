import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

// Real-data checks for tooling/src/check-workspace.ts (`bun run test:workspace`).

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = resolve(ROOT, "tooling/src/check-workspace.ts");

test.concurrent("workspace smoke passes on this checkout", async () => {
  const res = await run([process.execPath, SCRIPT], {
    cwd: ROOT,
    env: { CHECK_WORKSPACE_ROOT: undefined },
  });
  expect({ exitCode: res.exitCode, stdout: res.stdout }).toEqual({
    exitCode: 0,
    stdout: "check-workspace: ok\n",
  });
});

test.concurrent("workspace smoke names a missing workspace package.json", async () => {
  using sb = createSandbox({ files: { "package.json": '{ "name": "empty", "private": true }\n' } });
  const res = await run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { CHECK_WORKSPACE_ROOT: sb.project },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("check-workspace: missing packages/toolu-core/package.json");
});
