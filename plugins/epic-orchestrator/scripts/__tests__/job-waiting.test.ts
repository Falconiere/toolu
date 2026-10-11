/** The shipped Bun wrapper records a real refused admission and clears it later. */
import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "../../../../tools/toolu-conformance/src/harness/sandbox.ts";
import { bindWorktree } from "../../../../packages/toolu-core/src/resources/binding.ts";
import { acquireLease, releaseLease, writeJsonAtomic } from "../../../../packages/toolu-core/src/resources/resources.ts";

test("resource_status Bun job wrapper marks capacity refusal then clears it", async () => {
  using sandbox = createSandbox();
  const root = join(sandbox.root, "resources");
  const stateDir = join(sandbox.root, "epic-402");
  const initialized = Bun.spawnSync(["git", "init", sandbox.project], { stdout: "pipe", stderr: "pipe" });
  expect(initialized.exitCode).toBe(0);
  bindWorktree(root, sandbox.project, "repo-449", stateDir);
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const occupied = await acquireLease(root, { type: "job", key: "occupied", stateDir: "other" });
  const script = join(import.meta.dir, "../job.ts");
  const command = [process.execPath, script, "--", "--", "true"];
  const refused = Bun.spawnSync(command, { cwd: sandbox.project, stdout: "pipe", stderr: "pipe" });
  const marker = join(root, "waiting-jobs/repo-449.json");
  expect(refused.exitCode).not.toBe(0);
  expect(existsSync(marker)).toBe(true);
  const row = JSON.parse(readFileSync(marker, "utf8")) as { key: string; worktree: string };
  expect(row.key).toBe("repo-449");
  expect(row.worktree).toBe(sandbox.project);
  await releaseLease(root, occupied.token);
  const admitted = Bun.spawnSync(command, { cwd: sandbox.project, stdout: "pipe", stderr: "pipe" });
  expect(admitted.exitCode).toBe(0);
  expect(existsSync(marker)).toBe(false);
});
