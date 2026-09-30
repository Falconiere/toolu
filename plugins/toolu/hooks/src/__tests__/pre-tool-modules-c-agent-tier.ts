/**
 * agent-tier cases (#262): every `@test` of the deleted agent-tier.bats except
 * the hooks.json wiring test (the launcher gate covers it). The delegation
 * telemetry line under `<project>/.claude/tmp/telemetry/` is captured with each
 * case. Ledgers are written by hand at the default state path for `feat/x`.
 */
import type { Fixture } from "@toolu/conformance/harness/fixtures";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { gitIn, group, slugOf, writeIn, type GateCase } from "./pre-tool-modules-c-cases.ts";

const at = group({ hook: "agent-tier" });
const BRANCH = "feat/x";

type Step = { id: string; status: string; model: string };

const call =
  (toolName: string, toolInput: Record<string, unknown>): ((sb: Sandbox) => Fixture) =>
  () => ({ kind: "tool", event: "PreToolUse", toolName, toolInput });

/** A `feat/x` checkout, plus the ledger `steps`/`next` when given. */
function onBranch(steps?: Step[], next: string | null = null, stateDir = ".claude") {
  return (sb: Sandbox) => {
    gitIn(sb.project, ["checkout", "-q", "-b", BRANCH]);
    if (steps === undefined) return;
    const ledger = { version: 1, plan: "plan.md", steps, next };
    writeIn(
      sb.project,
      `${stateDir}/tmp/plan-ledger/${slugOf(BRANCH)}.json`,
      JSON.stringify(ledger),
    );
  };
}

/** s1 (haiku) is green, s2 (opus) is next. */
const S1_DONE: Step[] = [
  { id: "s1", status: "green", model: "haiku" },
  { id: "s2", status: "pending", model: "opus" },
];
/** s1 (haiku) is mid-run, s2 (opus) is pending. */
const S1_RUNNING: Step[] = [
  { id: "s1", status: "running", model: "haiku" },
  { id: "s2", status: "pending", model: "opus" },
];

const sonnet = call("Agent", { model: "sonnet" });

export const AGENT_TIER_CASES: GateCase[] = [
  at({
    name: "agent-tier: tool_name Agent appends a delegation event with model and subagent_type",
    setup: onBranch(),
    fixture: call("Agent", { model: "sonnet", subagent_type: "implementer" }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: tool_name TaskCreate records no event and prints nothing",
    setup: onBranch(),
    fixture: call("TaskCreate", { model: "opus" }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: tool_name Task appends a delegation event too (both client namings)",
    setup: onBranch(),
    fixture: call("Task", { model: "haiku", subagent_type: "quick-task" }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: tool_name spawn_agent maps Codex task_name to subagent_type telemetry",
    host: "codex",
    setup: onBranch(),
    fixture: call("spawn_agent", {
      model: "gpt-5.6-terra",
      task_name: "implementer",
      reasoning_effort: "medium",
    }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: no model param records model null and no advisory (inherit is legitimate)",
    setup: onBranch(S1_DONE, "s2"),
    fixture: call("Agent", { subagent_type: "fork" }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: no ledger at all records step_id and step_model null, no advisory",
    setup: onBranch(),
    fixture: call("Agent", { model: "opus" }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: model mismatch vs the next step's model, default advise, names both tiers",
    setup: onBranch(S1_DONE, "s2"),
    fixture: sonnet,
    expect: "advisory",
    has: ["s2", "opus", "sonnet"],
  }),
  at({
    name: "agent-tier: model mismatch under agentTier.mode=block is denied",
    setup: onBranch(S1_DONE, "s2"),
    config: { version: 1, agentTier: { mode: "block" } },
    fixture: sonnet,
    expect: "deny",
    has: ["s2", "opus"],
  }),
  at({
    name: "agent-tier: model mismatch under agentTier.mode=off is silent but still recorded",
    setup: onBranch(S1_DONE, "s2"),
    config: { version: 1, agentTier: { mode: "off" } },
    fixture: sonnet,
    expect: "silent",
  }),
  at({
    name: "agent-tier: matching model gives no advisory and no deny",
    setup: onBranch(S1_DONE, "s2"),
    fixture: call("Agent", { model: "opus" }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: a running step is preferred over next (joins s1/haiku, not s2/opus)",
    setup: onBranch(S1_RUNNING, "s1"),
    fixture: sonnet,
    expect: "advisory",
    has: ["s1", "haiku", "sonnet"],
    lacks: ["s2"],
  }),
  at({
    name: "agent-tier: no jq is a no-op in bash, the native hook records the delegation and decides normally",
    setup: onBranch(),
    without: ["jq"],
    fixture: call("Agent", { model: "opus" }),
    expect: "silent",
    deviation: "no jq: the native hook needs no jq; it records the delegation and decides normally",
  }),
  at({
    name: "agent-tier: malformed stdin JSON fails open with no output and no telemetry",
    setup: onBranch(),
    stdin: "not-json{",
    expect: "silent",
  }),
  at({
    name: "agent-tier: unreadable or corrupt ledger fails open, step_id and step_model null",
    setup: (sb) => {
      onBranch()(sb);
      writeIn(sb.project, `.claude/tmp/plan-ledger/${slugOf(BRANCH)}.json`, "this is not json {\n");
    },
    fixture: call("Agent", { model: "opus" }),
    expect: "silent",
  }),
  at({
    name: "agent-tier: codex spawn_agent with task_name and a model mismatch, default advise",
    host: "codex",
    setup: onBranch(S1_DONE, "s2", ".codex"),
    fixture: call("spawn_agent", {
      model: "gpt-5.6-terra",
      task_name: "implementer",
      reasoning_effort: "medium",
    }),
    expect: "advisory",
    has: ["s2", "opus", "gpt-5.6-terra"],
  }),
  at({
    name: "agent-tier: codex spawn_agent with a model mismatch under agentTier.mode=block",
    host: "codex",
    setup: onBranch(S1_DONE, "s2", ".codex"),
    config: { version: 1, agentTier: { mode: "block" } },
    fixture: call("spawn_agent", { model: "gpt-5.6-terra", task_name: "reviewer" }),
    expect: "deny",
    has: ["s2", "opus"],
  }),
];
