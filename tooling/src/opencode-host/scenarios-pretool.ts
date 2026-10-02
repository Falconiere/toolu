/** Real pinned-host proof that core pre-tool decisions stop side effects (#338). */
import { existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { runHost, toolStates } from "./host-run.ts";
import type { Scripts } from "./provider.ts";
import { openSession, PROBE_PLUGIN } from "./session.ts";
import { offeredTools, type ScenarioContext } from "./scenario.ts";
import {
  denied,
  ENV_BYTES,
  session,
  SMOKE_RUN_TIMEOUT_MS,
  type PretoolScenario,
} from "./pretool-shared.ts";

/** A host-only control: failure here occurs before toolu is present. */
async function baseline(ctx: ScenarioContext) {
  using s = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    config: () => ({ permission: { bash: "allow" } }),
    scripts: {
      "pretool.baseline": [
        { tool: "bash", args: { command: "touch baseline-marker", description: "baseline" } },
      ],
    },
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pretool.baseline"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const observed = {
    hostToolRan: toolStates(run).some(
      (state) => state.tool === "bash" && state.status === "completed",
    ),
    markerExists: s.exists("baseline-marker"),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function files(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { protectedFiles: { mode: "block" } },
      }),
    },
    scripts: (project): Scripts => ({
      "pretool.files": [
        { tool: "edit", args: { filePath: join(project, ".env"), oldString: "1", newString: "2" } },
        { tool: "write", args: { filePath: join(project, ".env"), content: "PWNED\n" } },
        { tool: "bash", args: { command: "touch allowed.txt", description: "allowed" } },
      ],
    }),
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pretool.files"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(run);
  const observed = {
    offered: offeredTools(s).join(","),
    editOffered: offeredTools(s).includes("edit"),
    writeOffered: offeredTools(s).includes("write"),
    editDenied: denied(states, "edit", /protected/i),
    writeDenied: denied(states, "write", /protected/i),
    envUnchanged: s.sb.read(".env") === ENV_BYTES,
    allowedBashRan:
      states.some((state) => state.tool === "bash" && state.status === "completed") &&
      s.exists("allowed.txt"),
  };
  return {
    pass:
      observed.editOffered &&
      observed.writeOffered &&
      observed.editDenied &&
      observed.writeDenied &&
      observed.envUnchanged &&
      observed.allowedBashRan,
    observed,
  };
}

async function patch(ctx: ScenarioContext) {
  using s = session(ctx, {
    model: "gpt-5-probe",
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { protectedFiles: { mode: "block" } },
      }),
    },
    scripts: (project): Scripts => ({
      "pretool.patch": [
        {
          tool: "apply_patch",
          args: {
            patchText: [
              "*** Begin Patch",
              `*** Add File: ${join(project, "added.txt")}`,
              "+new",
              `*** Update File: ${join(project, ".env")}`,
              "@@",
              "-SECRET=1",
              "+SECRET=2",
              "*** End Patch",
            ].join("\n"),
          },
        },
      ],
    }),
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pretool.patch"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const observed = {
    patchOffered: offeredTools(s).includes("apply_patch"),
    patchDenied: denied(toolStates(run), "apply_patch", /protected/i),
    envUnchanged: s.sb.read(".env") === ENV_BYTES,
    patchDestinationAbsent: !s.exists("added.txt"),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function shell(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { bashCommands: { mode: "block" }, qualityGate: { mode: "block" } },
      }),
      ".opencode/tmp/quality-gate-status.json": JSON.stringify({
        status: "failing",
        reason: "smoke gate",
      }),
    },
    scripts: {
      "pretool.shell": [
        {
          tool: "bash",
          args: {
            command: "touch unsafe-marker && node -e 'process.exit(0)'",
            description: "unsafe",
          },
        },
        {
          tool: "bash",
          args: {
            command: "touch commit-marker && git commit --allow-empty -m 'fix: smoke'",
            description: "commit",
          },
        },
      ],
    },
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pretool.shell"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(run).filter((state) => state.tool === "bash");
  const observed = {
    unsafeDenied: states.some(
      (state) => state.status === "error" && /deny rule/i.test(state.error ?? ""),
    ),
    commitDenied: states.some(
      (state) => state.status === "error" && /quality gate failing/i.test(state.error ?? ""),
    ),
    unsafeMarkerAbsent: !s.exists("unsafe-marker"),
    commitMarkerAbsent: !s.exists("commit-marker"),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function push(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { pushReview: { mode: "block" } },
      }),
    },
    scripts: {
      "pretool.push": [
        {
          tool: "bash",
          args: { command: "touch push-marker && git push origin feature", description: "push" },
        },
      ],
    },
  });
  s.sb.git("checkout", "-b", "feature");
  s.sb.write("feature.txt", "new\n");
  s.sb.git("add", "feature.txt");
  s.sb.git("commit", "-m", "feat: smoke");
  s.env.PUSH_REVIEW_BASE = "main";
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pretool.push"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const observed = {
    pushDenied: denied(toolStates(run), "bash", /Code review required before push/i),
    markerAbsent: !s.exists("push-marker"),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function mcp(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        mcp: { probe: false },
        gates: { mcpBlocker: { mode: "block" } },
      }),
    },
    config: (root) => ({
      mcp: {
        probe: {
          type: "local",
          command: [process.execPath, join(import.meta.dir, "mcp-server.ts")],
          environment: { MCP_MARKER: join(root, "mcp-marker.txt") },
        },
      },
    }),
    scripts: { "pretool.mcp": [{ tool: "probe_touch", args: { name: "blocked" } }] },
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pretool.mcp"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const observed = {
    mcpDenied: denied(toolStates(run), "probe_touch", /mcp\.probe=false/),
    markerAbsent: !existsSync(s.outside("mcp-marker.txt")),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function task(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { agentTier: { mode: "block" } },
      }),
    },
    scripts: {
      "pretool.task": [
        {
          tool: "task",
          args: {
            description: "child probe",
            prompt: "PROBE:pretool.child",
            subagent_type: "general",
            model: "probe/scripted",
          },
        },
      ],
      "pretool.child": [
        { tool: "bash", args: { command: "touch child-marker", description: "child" } },
      ],
    },
  });
  const ledger = s.outside("ledgers");
  mkdirSync(ledger);
  s.env.LEDGER_DIR = ledger;
  const ledgerFile = join(ledger, "main.json");
  await Bun.write(
    ledgerFile,
    JSON.stringify({
      next: "work",
      steps: [{ id: "work", status: "running", model: "expected-model" }],
    }),
  );
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:pretool.task"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const observed = {
    taskDenied: denied(toolStates(run), "task", /expects model tier "expected-model"/),
    childMarkerAbsent: !s.exists("child-marker"),
    childHookAbsent: !s.log().some((entry) => entry.kind === "before" && entry.tool === "bash"),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

export const PRETOOL_SCENARIOS: PretoolScenario[] = [
  { id: "pretool.baseline", run: baseline },
  { id: "pretool.files", run: files },
  { id: "pretool.patch", run: patch },
  { id: "pretool.shell", run: shell },
  { id: "pretool.push", run: push },
  { id: "pretool.mcp", run: mcp },
  { id: "pretool.task", run: task },
];
