import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { runPreToolBridge } from "../bridge.ts";
import { bridgeEnv, createProtectedFilesProject, REPO_ROOT } from "../test-helpers.ts";

test("runPreToolBridge real mod.sh protected .env edit yields deny or ask", async () => {
  const { projectRoot, envPath } = await createProtectedFilesProject();
  const fixturePath = join(REPO_ROOT, "tooling/fixtures/portable-core/protected-files-pre.json");
  const fixtureRaw: unknown = JSON.parse(readFileSync(fixturePath, "utf8"));
  const spread =
    typeof fixtureRaw === "object" && fixtureRaw !== null && !Array.isArray(fixtureRaw)
      ? fixtureRaw
      : {};

  const response = await runPreToolBridge(
    {
      ...spread,
      cwd: projectRoot,
      projectRoot,
      worktree: projectRoot,
      toolInput: { file_path: envPath },
    },
    { repoRoot: REPO_ROOT, env: bridgeEnv() },
  );

  expect(response.ok).toBe(true);
  expect(response.decision.kind === "deny" || response.decision.kind === "ask").toBe(true);
});
