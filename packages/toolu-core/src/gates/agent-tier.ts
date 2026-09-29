/**
 * The standalone `spawn_agent|Agent|Task` PreToolUse hook (#262), a port of
 * `pre-tools/agent-tier.sh`: every delegation is recorded as `delegation`
 * telemetry, joined with the plan-ledger step it serves (the running step,
 * else the ledger's `next`), and a call whose `model` differs from that
 * step's declared model gets the `agentTier` gate-mode decision. A missing
 * call model inherits the tier and is never flagged. It fails open: a
 * telemetry or ledger problem never blocks a delegation.
 */
import { loadConfig } from "../config/config-load.ts";
import { gateDecision, gateMode } from "../config/gate-mode.ts";
import { branchSlug } from "../detect/detect-branch.ts";
import { toolAvailable } from "../detect/detect-tools.ts";
import { encodeDecision } from "../host/host-encode.ts";
import { detectHost } from "../host/host-detect.ts";
import { envValue, type HostEnv, type HostName } from "../host/host-name.ts";
import { gitToplevel, projectStateDir } from "../host/host-roots.ts";
import {
  alt,
  eachOptional,
  get,
  JqError,
  jqEquals,
  parseJson,
  raw,
  type Json,
} from "../ledger/ledger-jq.ts";
import { isFile } from "../ledger/ledger-parse.ts";
import { readJson } from "../ledger/verdict-gates.ts";
import { telemetryAppend } from "../state/telemetry.ts";
import { gitAt } from "./push-target.ts";

export type AgentTierResult = { stdout: string; stderr: string; exitCode: number };

const SILENT: AgentTierResult = { stdout: "", stderr: "", exitCode: 0 };

const DELEGATING_TOOLS = new Set(["Agent", "Task", "spawn_agent"]);

/** `$(jq -r 'EXPR // empty' 2>/dev/null || true)`: absent, null, false or a jq error is `""`. */
function text(fn: () => Json): string {
  try {
    const value = fn();
    return value === null || value === false ? "" : raw(value).replace(/\n+$/, "");
  } catch (error) {
    if (error instanceof JqError) return "";
    throw error;
  }
}

/** `.tool_input.a // .tool_input.b // ... // empty`; jq's `//` does not catch an error on its left. */
function inputField(doc: Json, keys: readonly string[]): string {
  return text(() => {
    const input = get(doc, "tool_input");
    return keys.reduce<Json>((found, key) => alt(found, get(input, key)), null);
  });
}

/** `.steps[]?`: nothing when `.steps` cannot be indexed or iterated. */
function stepsOf(ledger: Json): Json[] {
  try {
    return eachOptional(get(ledger, "steps"));
  } catch (error) {
    if (error instanceof JqError) return [];
    throw error;
  }
}

/** The running step's id, else the ledger's `next`; then that step's declared `model`. */
function joinStep(ledger: Json): { stepId: string; stepModel: string } {
  const stepId = text(() => {
    const running = stepsOf(ledger).find((step) => jqEquals(get(step, "status"), "running"));
    return alt(running === undefined ? null : get(running, "id"), get(ledger, "next"));
  });
  if (stepId === "") return { stepId, stepModel: "" };
  const stepModel = text(() => {
    const step = stepsOf(ledger).find((s) => jqEquals(get(s, "id"), stepId));
    return step === undefined ? null : get(step, "model");
  });
  return { stepId, stepModel };
}

/** The ledger join for `root`'s checked-out branch; empty when there is no readable ledger. */
function ledgerJoin(root: string, env: HostEnv, host: HostName) {
  const none = { stepId: "", stepModel: "" };
  const branch = (gitAt(root, ["rev-parse", "--abbrev-ref", "HEAD"], env) ?? "").trim();
  if (branch === "" || branch === "HEAD") return none;
  const dir = envValue(env, "LEDGER_DIR") ?? projectStateDir("plan-ledger", { env, host, root });
  const file = `${dir ?? ""}/${branchSlug(branch)}.json`;
  if (!isFile(file)) return none;
  const ledger = readJson(file);
  // `jq -e .` rejects an unparseable file, and a bare `null` or `false`.
  if (ledger === undefined || ledger === null || ledger === false) return none;
  return joinStep(ledger);
}

const orNull = (value: string): string | null => (value === "" ? null : value);

function run(stdin: string, env: HostEnv, cwd: string): AgentTierResult {
  const doc = parseJson(stdin) ?? null;
  const toolName = text(() => get(doc, "tool_name"));
  if (!DELEGATING_TOOLS.has(toolName) || !toolAvailable("git", env)) return SILENT;
  const root = gitToplevel(env, cwd);
  if (root === undefined) return SILENT;
  const model = inputField(doc, ["model"]);
  const subagent = inputField(doc, ["subagent_type", "task_name", "agent_type"]);
  const effort = inputField(doc, ["reasoning_effort", "reasoningEffort"]);
  const host = detectHost({ env });
  const { stepId, stepModel } = ledgerJoin(root, env, host);
  const warnings: string[] = [];
  const warn = (line: string) => warnings.push(`toolu-config: ${line}\n`);
  const extras = {
    model: orNull(model),
    subagent_type: orNull(subagent),
    reasoning_effort: orNull(effort),
    step_id: orNull(stepId),
    step_model: orNull(stepModel),
  };
  telemetryAppend(root, "delegation", extras, { env, host, warn });
  if (stepModel === "" || model === "" || stepModel === model) {
    return { ...SILENT, stderr: warnings.join("") };
  }
  const mode = gateMode(loadConfig({ env, host, cwd, warn }), "agentTier", { host });
  const reason = `plan step "${stepId}" expects model tier "${stepModel}" but this delegation used "${model}"`;
  const decision = gateDecision(mode, reason);
  const out =
    decision === null
      ? undefined
      : encodeDecision(host === "codex" ? "codex" : "claude", "tool/pre", decision);
  const stdout = out?.kind === "command" ? out.stdout : "";
  return { stdout, stderr: warnings.join(""), exitCode: 0 };
}

/** One hook call: the host's stdin in, what the host must see out. Never blocks on its own failure. */
export function agentTierHook(
  stdin: string,
  options: { env?: HostEnv; cwd?: string } = {},
): AgentTierResult {
  try {
    return run(stdin, options.env ?? process.env, options.cwd ?? process.cwd());
  } catch {
    return SILENT;
  }
}
