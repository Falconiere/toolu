/**
 * Import-cost probe (#257, AC-7): one fresh process runs `runRegistry` over the
 * post-tool registry under `TOOLU_CONFIG_DIR` and prints each module's import
 * plus run time. Spawned by `registry-import-cost.test.ts`.
 */
import { runRegistry } from "../../registry-run.ts";

const configRoot = process.env.TOOLU_CONFIG_DIR ?? "";
const outcomes = await runRegistry(
  {
    type: "tool/post",
    sessionId: "s",
    cwd: configRoot,
    projectRoot: configRoot,
    worktree: configRoot,
    toolCallId: "t",
    toolName: "Edit",
    toolInput: {},
  },
  { host: "claude", env: process.env, configRoot, projectRoot: configRoot, raw: {} },
);
const ms = outcomes.map((o) => (o.status === "skipped" ? -1 : o.ms));
process.stdout.write(`${JSON.stringify({ ms, statuses: outcomes.map((o) => o.status) })}\n`);
