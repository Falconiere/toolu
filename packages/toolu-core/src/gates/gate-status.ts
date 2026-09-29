/**
 * PostToolUse gate-status (#259), the native port of
 * `post-tools/modules/gate-status.sh`: after a Bash/Shell call that ran a
 * quality command, record the command channel of the quality gate
 * (`__global__`, source `gate-status-hook`) as failing or clear it.
 *
 * Where it goes past bash (#283 items 6 and 7): a quality command must be one
 * the line runs, not text that names it, and the line's exit status must be
 * that command's own (`exitProves`). A failure is recorded when some quality
 * command's status is the line's; a pass only when every quality command's
 * is, so `bun test | tail; bun run lint` exiting 0 never vouches for the
 * tests. Anything else leaves the slot as it was.
 */
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Decision } from "../decision/decision.ts";
import type { ToolModule } from "../dispatch/dispatch-context.ts";
import { projectStateRoot } from "../host/host-roots.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { GLOBAL_GATE_KEY, clearGateFile, recordGateFailure } from "../state/gate-file.ts";
import { isoSeconds, toJqJson } from "../state/state-io.ts";
import { commandAnalysis, isShellTool } from "./command-analysis.ts";
import { qualityCommands } from "./quality-command.ts";
import { toolCommand, toolExitStatus } from "./tool-exit.ts";

const SOURCE = "gate-status-hook";
const ALLOW: Decision = { kind: "allow" };

/**
 * The advisory bash prints. Its `\n` is two characters: bash passes the
 * double-quoted `\n` to `jq --arg` unexpanded.
 */
function failingContext(command: string, status: string): string {
  return `Global quality gate failing. Fix all errors/warnings/tests before new tasks.\\nFailed: ${command} (exit ${status})`;
}

/** `$GATE_DIR`: `<PROJECT_ROOT>/<.host>/tmp`, created as bash's `mkdir -p` does. */
function gateDir(ctx: RegistryContext): string {
  const dir = projectStateRoot({ env: ctx.env, host: ctx.host, root: ctx.projectRoot });
  if (dir === undefined) throw new Error("gate-status: no project state root");
  mkdirSync(dir, { recursive: true });
  return dir;
}

/**
 * The first-ever pass, with nothing tracked yet: `jq -n ... > "$GATE_FILE"`,
 * pretty, its source the command. A plain write, as bash's redirect is.
 */
function writeFirstPass(gateFile: string, command: string): void {
  const doc = { status: "passing", source: command, updatedAt: isoSeconds(new Date()) };
  writeFileSync(gateFile, `${toJqJson(doc, true)}\n`);
}

function decide(event: RegistryHookEvent, ctx: RegistryContext): Decision {
  if (!isShellTool(event)) return ALLOW;
  const gateFile = join(gateDir(ctx), "quality-gate-status.json");
  const quality = qualityCommands(commandAnalysis(event, ctx));
  if (quality.length === 0) return ALLOW;
  const command = toolCommand(ctx.raw);
  const status = toolExitStatus(ctx.raw);
  const state = { env: ctx.env, host: ctx.host };
  if (/^[0-9]+$/.test(status) && Number(status) !== 0) {
    if (!quality.some((q) => q.command.exitProves)) return ALLOW;
    const reason = `Quality command failed: ${command} (exit ${status})`;
    recordGateFailure(gateFile, GLOBAL_GATE_KEY, SOURCE, reason, "", state);
    return { kind: "advisory", message: failingContext(command, status) };
  }
  if (status === "0" && quality.every((q) => q.command.exitProves)) {
    clearGateFile(gateFile, GLOBAL_GATE_KEY, SOURCE, state);
    if (!existsSync(gateFile)) writeFirstPass(gateFile, command);
  }
  return ALLOW;
}

/** The built-in `gate-status.sh`, native. */
export const gateStatusModule: ToolModule = {
  kind: "native",
  name: "gate-status.sh",
  run: (event, ctx) => Promise.resolve(decide(event, ctx)),
};
