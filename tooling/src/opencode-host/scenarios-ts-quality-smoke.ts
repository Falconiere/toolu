/** Pinned-host proof of selected TypeScript quality checks on native OpenCode tools (#352). */
import { join } from "node:path";
import { runHost, toolStates } from "./host-run.ts";
import { finalMessages, type ScenarioContext } from "./scenario.ts";
import { session, SMOKE_RUN_TIMEOUT_MS, type PretoolScenario } from "./pretool-shared.ts";
import { disabledQuality, qualityGate as gate, qualityProject } from "./quality-smoke-shared.ts";
import { prepareSdk, verdict } from "./scenarios-posttool-smoke.ts";

const FILES = {
  "package.json": '{"name":"ts-quality-smoke","private":true}\n',
  "bun.lock": "lock marker\n",
  "tsconfig.json": "{}\n",
};
const PROJECT = qualityProject(["ts-quality"], FILES);

async function editQuality(ctx: ScenarioContext) {
  const bad = "function f() { try { return 1; } catch {} }\n";
  const clean = "const answer = 42;\n";
  using s = session(ctx, {
    files: PROJECT,
    scripts: (project) => ({
      "tsquality.edit": [
        { tool: "write", args: { filePath: join(project, "bad.tsx"), content: bad } },
        {
          tool: "bash",
          args: {
            command: "touch commit-marker && git commit --allow-empty -m 'fix: invalid'",
            description: "commit after invalid TypeScript edit",
          },
        },
        {
          tool: "bash",
          args: {
            command: "touch push-marker && git push origin main",
            description: "push after invalid TypeScript edit",
          },
        },
        {
          tool: "edit",
          args: { filePath: join(project, "bad.tsx"), oldString: bad, newString: clean },
        },
        {
          tool: "write",
          args: { filePath: join(project, "notes.md"), content: "console.log('text only')\n" },
        },
      ],
    }),
  });
  s.sb.git("add", "tsconfig.json");
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:tsquality.edit"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(host);
  const messages = finalMessages(s, "tool");
  const deniedBash = states.filter((state) => state.tool === "bash" && state.status === "error");
  const observed = {
    writeCompleted: states.some((state) => state.tool === "write" && state.status === "completed"),
    diagnosticVisible: messages.some((message) => message.includes("Empty catch block")),
    commitDenied:
      deniedBash.length === 2 && /quality gate failing/i.test(deniedBash[0]?.error ?? ""),
    pushDenied: /quality gate failing/i.test(deniedBash[1]?.error ?? ""),
    markersAbsent: !s.exists("commit-marker") && !s.exists("push-marker"),
    editCompleted: states.some((state) => state.tool === "edit" && state.status === "completed"),
    recovered: gate(s)?.status === "passing" && s.sb.read("bad.tsx") === clean,
    unrelatedIgnored:
      s.sb.read("notes.md") === "console.log('text only')\n" &&
      !(messages.at(-1)?.includes("QUALITY VIOLATION") ?? false),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}

function patchText(project: string): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${join(project, "old.tsx")}`,
    `*** Move to: ${join(project, "moved.tsx")}`,
    "@@",
    '-console.log("bad");',
    "+function f() { try { return 1; } catch {} }",
    `*** Delete File: ${join(project, "removed.ts")}`,
    `*** Add File: ${join(project, "added.ts")}`,
    '+console.log("new bad");',
    `*** Add File: ${join(project, "notes.md")}`,
    "+unrelated",
    "*** End Patch",
  ].join("\n");
}

function seedPatch(project: string): string {
  return [
    "*** Begin Patch",
    `*** Add File: ${join(project, "old.tsx")}`,
    '+console.log("bad");',
    `*** Add File: ${join(project, "removed.ts")}`,
    '+console.log("bad");',
    "*** End Patch",
  ].join("\n");
}

async function patchQuality(ctx: ScenarioContext) {
  using s = session(ctx, {
    model: "gpt-5-probe",
    files: PROJECT,
    scripts: (project) => ({
      "tsquality.patch": [
        { tool: "apply_patch", args: { patchText: seedPatch(project) } },
        { tool: "apply_patch", args: { patchText: patchText(project) } },
      ],
    }),
  });
  s.sb.git("add", "tsconfig.json");
  await prepareSdk(s, ctx.cacheRoot);
  const host = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:tsquality.patch"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const states = toolStates(host);
  const messages = finalMessages(s, "tool");
  const entries = gate(s)?.entries ?? {};
  const expected = [join(s.sb.project, "added.ts"), join(s.sb.project, "moved.tsx")].toSorted();
  const observed = {
    patchCompleted: states.some(
      (state) => state.tool === "apply_patch" && state.status === "completed",
    ),
    moveApplied:
      !s.exists("old.tsx") &&
      s.exists("moved.tsx") &&
      s.sb.read("moved.tsx") === "function f() { try { return 1; } catch {} }\n",
    deleteApplied: !s.exists("removed.ts"),
    addApplied: s.exists("added.ts") && s.sb.read("added.ts") === 'console.log("new bad");\n',
    unrelatedApplied: s.exists("notes.md") && s.sb.read("notes.md") === "unrelated\n",
    exactGateEntries:
      gate(s)?.status === "failing" &&
      JSON.stringify(Object.keys(entries).toSorted()) === JSON.stringify(expected) &&
      expected.every((path) => entries[path]?.source === "ts-quality-hook"),
    bothVisible: messages.some(
      (message) =>
        message.includes("Empty catch block") && message.includes("Forbidden console.log"),
    ),
    hostSucceeded: host.exitCode === 0,
  };
  return verdict(observed, { messages, states, stderr: host.stderr });
}

export const TS_QUALITY_SCENARIOS: PretoolScenario[] = [
  { id: "tsquality.edit", run: editQuality },
  { id: "tsquality.patch", run: patchQuality },
  {
    id: "tsquality.disabled",
    run: (ctx) =>
      disabledQuality(ctx, {
        id: "tsquality.disabled",
        files: FILES,
        file: "bad.ts",
        content: 'console.log("bad");\n',
        prepare: (s) => s.sb.git("add", "tsconfig.json"),
      }),
  },
];
