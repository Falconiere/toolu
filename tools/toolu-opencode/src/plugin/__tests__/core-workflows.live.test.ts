/**
 * Live core-workflow proof (#358, AC-10), reported apart from the hermetic
 * suite. `TOOLU_LIVE_OPENCODE=1` drives the pinned host with a scripted model:
 * it loads the review skill through the native `skill` tool, sees its push
 * denied, records the review with the skill's command and pushes; then loads
 * the debug skill, runs its collector on a failing test, and delegates to
 * `toolu-quick-task` through `task`.
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { runHost, toolStates } from "../../../../../tooling/src/opencode-host/host-run.ts";
import {
  hostCacheDir,
  resolveHostBinary,
} from "../../../../../tooling/src/opencode-host/install.ts";
import { contractPaths } from "../../../../../tooling/src/opencode-host/results.ts";
import { allRequestText } from "../../../../../tooling/src/opencode-host/scenario.ts";
import {
  ROOT,
  diagnostics,
  installShim,
} from "../../../../../tooling/src/opencode-host/scenarios-entry.ts";
import { PinSchema, readJson } from "../../../../../tooling/src/opencode-host/schema.ts";
import { openSession } from "../../../../../tooling/src/opencode-host/session.ts";
import {
  BRANCH,
  FAILING_NAME,
  FAILING_TEST,
  debugTestfailCommand,
  reviewProject,
  writeStateCommand,
} from "./core-fixtures.ts";

const PROMPT = "PROBE:core.workflows review, push, debug and delegate";
const PUSH = `git push origin ${BRANCH}`;
const bash = (command: string) => ({ tool: "bash", args: { command, description: "core" } });

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "the pinned host runs the review, debug and delegation workflows through native tools",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    using session = openSession(cacheRoot, {
      config: () => ({ permission: { bash: "allow" } }),
      scripts: {
        "core.workflows": [
          { tool: "skill", args: { name: "toolu-review-review" } },
          bash(PUSH),
          bash(writeStateCommand(0)),
          bash(PUSH),
          { tool: "skill", args: { name: "toolu-debug" } },
          bash(debugTestfailCommand()),
          {
            tool: "task",
            args: {
              description: "list files",
              prompt: "List the files in the project root.",
              subagent_type: "toolu-quick-task",
            },
          },
        ],
      },
    });
    const remote = reviewProject(session.sb, { pushReview: { mode: "block" } });
    session.sb.write("debug/fail.test.ts", FAILING_TEST);
    installShim(session);
    session.env.TOOLU_BUN = process.execPath;
    session.env.TOOLU_REPO_ROOT = ROOT;

    const hostRun = await runHost(host.bin, session, ["--print-logs", PROMPT]);
    expect(hostRun.exitCode).toBe(0);
    expect(diagnostics(hostRun.stderr, "toolu: ready")).toBe(1);
    const states = toolStates(hostRun);
    expect(states.map(({ tool, status }) => `${tool}:${status}`)).toEqual([
      "skill:completed",
      "bash:error",
      "bash:completed",
      "bash:completed",
      "skill:completed",
      "bash:completed",
      "task:completed",
    ]);
    expect(states[1]?.error).toContain("Code review required before push");
    const pushed = session.sb.git("--git-dir", remote, "rev-parse", `refs/heads/${BRANCH}`).trim();
    expect(pushed).toBe(session.sb.git("rev-parse", "HEAD").trim());
    const captured = allRequestText(session);
    expect(captured).toContain(`  - ${FAILING_NAME}`);
    expect(captured).toMatch(/fail\.test\.ts:4:\d+/);
    process.stdout.write(
      `${JSON.stringify({ host: host.version, tools: states.map((s) => `${s.tool}:${s.status}`), pushed })}\n`,
    );
  },
  900_000,
);
