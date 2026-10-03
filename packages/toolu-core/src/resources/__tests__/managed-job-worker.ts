/** Separate owner process used to verify managed-job crash and signal cleanup. */
import { runManagedJob } from "../jobs.ts";
import type { ResourceBinding } from "../binding.ts";

const [root, worktree, marker] = process.argv.slice(2);
if (!root || !worktree || !marker) throw new Error("root, worktree and marker required");
const binding: ResourceBinding = {
  version: 1,
  root,
  key: "issue-376",
  stateDir: `${root}/epic`,
  worktree,
};
await runManagedJob(
  [
    process.execPath,
    "-e",
    `await Bun.write(${JSON.stringify(marker)}, "started"); await Bun.sleep(30_000);`,
  ],
  binding,
  { timeoutMs: 30_000 },
);
