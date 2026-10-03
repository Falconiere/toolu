/** Pinned-host proof that OpenCode completes tools before toolu runs post checks. */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { run } from "@toolu/conformance/harness/spawn";
import { runHost, toolStates } from "./host-run.ts";
import type { Scripts } from "./provider.ts";
import { finalMessages, type ScenarioContext } from "./scenario.ts";
import { denied, session, SMOKE_RUN_TIMEOUT_MS, type PretoolScenario } from "./pretool-shared.ts";
import { contractPaths } from "./results.ts";
import { ContractError, PinSchema, readJson } from "./schema.ts";

const GATE = ".opencode/tmp/quality-gate-status.json";
const sdk = readJson(contractPaths().pin, PinSchema).sdk;
const Gate = z.object({
  status: z.string(),
  file: z.string().optional(),
  source: z.string().optional(),
});

function gate(s: ReturnType<typeof session>): z.infer<typeof Gate> | null {
  if (!s.exists(GATE)) return null;
  const raw: unknown = JSON.parse(s.sb.read(GATE));
  const parsed = Gate.safeParse(raw);
  return parsed.success ? parsed.data : null;
}

async function prepareSdkDir(s: ReturnType<typeof session>, dir: string): Promise<void> {
  const spec = `${sdk.package}@${sdk.version}`;
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "package.json"),
    `${JSON.stringify({ name: "opencode-smoke-sdk", private: true })}\n`,
  );
  const result = await run([process.execPath, "add", "--exact", spec], {
    cwd: dir,
    env: { ...s.env, PWD: dir },
    timeoutMs: 300_000,
  });
  if (result.exitCode !== 0 || result.timedOut) {
    throw new ContractError(
      `isolated SDK install failed in ${dir} (exit ${result.exitCode}, timedOut ${result.timedOut}): ${result.stderr.slice(-1000)}`,
    );
  }
  const manifest: unknown = JSON.parse(
    readFileSync(join(dir, "node_modules/@opencode-ai/plugin/package.json"), "utf8"),
  );
  if (z.object({ version: z.string() }).parse(manifest).version !== sdk.version) {
    throw new ContractError(`isolated SDK version mismatch in ${dir}`);
  }
}

/** Provision the host's pinned SDK in both isolated config dirs before startup. */
export async function prepareSdk(s: ReturnType<typeof session>, cacheRoot: string): Promise<void> {
  s.env.BUN_INSTALL_CACHE_DIR = join(cacheRoot, "bun-install-cache");
  mkdirSync(s.env.BUN_INSTALL_CACHE_DIR, { recursive: true });
  await prepareSdkDir(s, join(s.sb.home, ".config/opencode"));
  await prepareSdkDir(s, join(s.sb.project, ".opencode"));
}

function verdict(
  observed: Record<string, boolean>,
  details: { messages: string[]; states: ReturnType<typeof toolStates>; stderr: string },
) {
  const pass = Object.values(observed).every(Boolean);
  return {
    pass,
    observed: pass ? observed : { ...observed, diagnostic: JSON.stringify(details).slice(0, 7000) },
  };
}

async function shellQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      "package.json": '{"name":"post-smoke","private":true,"scripts":{"test":"exit 3"}}\n',
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { qualityGate: { mode: "block" } },
      }),
    },
    scripts: {
      "posttool.shell": [
        { tool: "bash", args: { command: "bun run test", description: "failing quality" } },
        {
          tool: "bash",
          args: {
            command: "touch commit-marker && git commit --allow-empty -m 'fix: smoke'",
            description: "commit after quality failure",
          },
        },
        {
          tool: "bash",
          args: {
            command: "touch push-marker && git push origin main",
            description: "push after failure",
          },
        },
        { tool: "read", args: { filePath: "missing.txt" } },
      ],
    },
  });
  await prepareSdk(s, ctx.cacheRoot);
  const hostRun = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:posttool.shell"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(hostRun);
  const messages = finalMessages(s, "tool");
  const observed = {
    qualityRan: states.some((state) => state.tool === "bash" && state.status === "completed"),
    failureRecorded: gate(s)?.status === "failing" && gate(s)?.source === "gate-status-hook",
    failureVisible: messages[0]?.includes("Global quality gate failing") ?? false,
    commitDenied: denied(states, "bash", /quality gate failing/i),
    pushDenied:
      states.filter((state) => state.tool === "bash" && state.status === "error").length >= 2,
    markersAbsent: !s.exists("commit-marker") && !s.exists("push-marker"),
    missingReadError: denied(states, "read", /File not found/i),
    noAfterForRead: !s.log().some((entry) => entry.kind === "after" && entry.tool === "read"),
  };
  return verdict(observed, { messages, states, stderr: hostRun.stderr });
}

async function editQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu/plugins.json": JSON.stringify({
        version: 1,
        enabled: ["toolu", "ts-quality"],
      }),
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { qualityGate: { mode: "block" } },
      }),
      "package.json": '{"name":"post-edit-smoke","private":true}\n',
      "bun.lock": "lock marker\n",
      "tsconfig.json": "{}\n",
    },
    scripts: (project): Scripts => ({
      "posttool.edit": [
        {
          tool: "write",
          args: { filePath: join(project, "bad.ts"), content: 'console.log("bad");\n' },
        },
        {
          tool: "bash",
          args: {
            command: "touch edit-commit-marker && git commit --allow-empty -m 'fix: smoke'",
            description: "commit after edit",
          },
        },
      ],
    }),
  });
  s.sb.git("add", "tsconfig.json");
  await prepareSdk(s, ctx.cacheRoot);
  const hostRun = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:posttool.edit"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(hostRun);
  const messages = finalMessages(s, "tool");
  const observed = {
    writeCompleted: states.some((state) => state.tool === "write" && state.status === "completed"),
    bytesChanged: s.exists("bad.ts") && s.sb.read("bad.ts") === 'console.log("bad");\n',
    fileFailure: gate(s)?.status === "failing" && gate(s)?.file === join(s.sb.project, "bad.ts"),
    violationVisible: messages[0]?.includes("QUALITY VIOLATION") ?? false,
    commitDenied: denied(states, "bash", /quality gate failing/i),
    markerAbsent: !s.exists("edit-commit-marker"),
  };
  return verdict(observed, { messages, states, stderr: hostRun.stderr });
}

function patchText(project: string): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${join(project, "source.ts")}`,
    `*** Move to: ${join(project, "moved.ts")}`,
    "@@",
    "-export const move = 1;",
    "+export const move = 2;",
    `*** Delete File: ${join(project, "gone.ts")}`,
    `*** Add File: ${join(project, "bad.ts")}`,
    '+console.log("bad");',
    "*** End Patch",
  ].join("\n");
}

async function patchQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    model: "gpt-5-probe",
    files: {
      ".opencode/toolu/plugins.json": JSON.stringify({
        version: 1,
        enabled: ["toolu", "ts-quality"],
      }),
      "package.json": '{"name":"post-patch-smoke","private":true}\n',
      "bun.lock": "lock marker\n",
      "tsconfig.json": "{}\n",
      "source.ts": "export const move = 1;\n",
      "gone.ts": "export const gone = 1;\n",
    },
    scripts: (project): Scripts => ({
      "posttool.patch": [
        {
          tool: "apply_patch",
          args: { patchText: patchText(project) },
        },
      ],
    }),
  });
  s.sb.git("add", "tsconfig.json");
  await prepareSdk(s, ctx.cacheRoot);
  const hostRun = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:posttool.patch"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(hostRun);
  const messages = finalMessages(s, "tool");
  const observed = {
    patchCompleted: states.some(
      (state) => state.tool === "apply_patch" && state.status === "completed",
    ),
    moveApplied:
      !s.exists("source.ts") &&
      s.exists("moved.ts") &&
      s.sb.read("moved.ts") === "export const move = 2;\n",
    deleteApplied: !s.exists("gone.ts"),
    addApplied: s.exists("bad.ts") && s.sb.read("bad.ts") === 'console.log("bad");\n',
    fileFailure: gate(s)?.status === "failing" && gate(s)?.file === join(s.sb.project, "bad.ts"),
    violationVisible: messages[0]?.includes("QUALITY VIOLATION") ?? false,
  };
  return verdict(observed, { messages, states, stderr: hostRun.stderr });
}

export const POSTTOOL_SCENARIOS: PretoolScenario[] = [
  { id: "posttool.shell", run: shellQuality },
  { id: "posttool.edit", run: editQuality },
  { id: "posttool.patch", run: patchQuality },
];
