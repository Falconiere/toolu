/** Model-visible advice and other-plugin composition on the pinned host. */
import { runHost, toolStates } from "./host-run.ts";
import { denied, session, SMOKE_RUN_TIMEOUT_MS, type PretoolScenario } from "./pretool-shared.ts";
import { finalMessages, type ScenarioContext } from "./scenario.ts";

const ENABLE_AST = JSON.stringify({ version: 1, enabled: ["toolu", "ast-grep"] });

async function judgementAdvice(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        gates: { qualityGate: { mode: "ask" } },
      }),
      ".opencode/tmp/quality-gate-status.json": JSON.stringify({
        status: "failing",
        reason: "permission smoke gate",
      }),
    },
    scripts: {
      "permissions.judgement": [
        {
          tool: "bash",
          args: {
            command: "touch judgement-marker && git commit --allow-empty -m 'fix: smoke'",
            description: "judgement advice",
          },
        },
      ],
    },
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:permissions.judgement"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const messages = finalMessages(s, "tool");
  const observed = {
    commandRan:
      s.exists("judgement-marker") &&
      toolStates(run).some((state) => state.tool === "bash" && state.status === "completed"),
    adviceVisible: messages.some(
      (message) =>
        message.includes("toolu advisory") && message.includes("quality gate is failing"),
    ),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function registryAdvice(ctx: ScenarioContext) {
  using s = session(ctx, {
    files: {
      ".opencode/toolu/plugins.json": ENABLE_AST,
      "src/Foo.ts": "class Foo {}\n",
    },
    scripts: {
      "permissions.registry-advice": [
        { tool: "grep", args: { pattern: "class Foo" } },
        { tool: "bash", args: { command: "touch second-marker", description: "plain call" } },
      ],
    },
  });
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:permissions.registry-advice"],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const messages = finalMessages(s, "tool");
  const observed = {
    bothRan:
      s.exists("second-marker") &&
      toolStates(run).filter((state) => state.status === "completed").length >= 2,
    firstHasAdvice:
      messages[0] !== undefined &&
      messages[0].includes("toolu advisory") &&
      messages[0].includes("ast-grep"),
    secondLacksAdvice: messages[1] !== undefined && !messages[1].includes("toolu advisory"),
  };
  const pass = Object.values(observed).every(Boolean);
  if (pass) return { pass, observed };
  return {
    pass,
    observed: {
      ...observed,
      diagnostic: JSON.stringify({
        exitCode: run.exitCode,
        states: toolStates(run),
        messages,
        stderr: run.stderr,
        log: s.log(),
      }).slice(0, 8000),
    },
  };
}

const OTHER_PLUGIN = `
import { appendFileSync } from "node:fs";
export default {
  id: "other-denier",
  server: async () => ({
    "tool.execute.before": async (input, output) => {
      if (!JSON.stringify(output.args).includes("plugin-marker")) return;
      const path = process.env.TOOLU_PROBE_LOG;
      if (path) appendFileSync(path, JSON.stringify({ kind: "other-before", tool: input.tool }) + "\\n");
      throw new Error("other-plugin denied");
    },
  }),
};
`;

async function otherPlugin(ctx: ScenarioContext, name: "aaa-deny" | "zzz-deny") {
  using s = session(ctx, {
    files: {
      ".opencode/toolu/plugins.json": ENABLE_AST,
      ...(name === "aaa-deny" ? { ".opencode/plugins/aaa-deny.ts": OTHER_PLUGIN } : {}),
    },
    scripts: {
      [`permissions.${name}`]: [
        {
          tool: "bash",
          args: { command: "rg TODO src && touch plugin-marker", description: "other deny" },
        },
        { tool: "bash", args: { command: "touch allowed-after-marker", description: "after" } },
      ],
    },
  });
  if (name === "zzz-deny") s.sb.write(".opencode/plugins/zzz-deny.ts", OTHER_PLUGIN);
  const run = await runHost(
    ctx.bin,
    s,
    ["--print-logs", `PROBE:permissions.${name}`],
    SMOKE_RUN_TIMEOUT_MS,
  );
  const log = s.log();
  const other = log.findIndex((entry) => entry.kind === "other-before");
  const probe = log.findIndex((entry) => entry.kind === "before" && entry.tool === "bash");
  const messages = finalMessages(s, "tool");
  const observed = {
    denied: denied(toolStates(run), "bash", /other-plugin denied/),
    markerAbsent: !s.exists("plugin-marker"),
    laterRan: s.exists("allowed-after-marker"),
    loadOrder: other >= 0 && probe >= 0 && (name === "aaa-deny" ? other < probe : other > probe),
    noStaleAdvice: !messages.some((message) => message.includes("toolu advisory")),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

export const ADVICE_SCENARIOS: PretoolScenario[] = [
  { id: "permissions.judgement", run: judgementAdvice },
  { id: "permissions.registry-advice", run: registryAdvice },
  { id: "permissions.other-before", run: (ctx) => otherPlugin(ctx, "aaa-deny") },
  { id: "permissions.other-after", run: (ctx) => otherPlugin(ctx, "zzz-deny") },
];
