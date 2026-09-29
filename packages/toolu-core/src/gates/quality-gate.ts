/**
 * quality-gate (#261): while the gate file says "failing", stop the two actions
 * that let a red build leave the working tree, `git commit` and `git push`, and
 * nothing else. A port of `pre-tools/modules/quality-gate.sh`. Commits and
 * pushes are found in the parsed command (`@toolu/core/detect` on
 * `@toolu/core/shell`), so `timeout 120 git push`, `xargs git push`, a commit
 * behind `sudo` or inside `bash -c` are seen too (#283 item 8).
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { isJsonObject } from "../config/config-load.ts";
import { gateDecision, gateMode, type GateMode } from "../config/gate-mode.ts";
import type { Decision } from "../decision/decision.ts";
import { isGitCommit, isGitPush } from "../detect/detect-git.ts";
import { toolAvailable } from "../detect/detect-tools.ts";
import { childEnv, envValue, type HostEnv } from "../host/host-name.ts";
import { gitToplevel, projectStateRoot } from "../host/host-roots.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { shellAnalysisOf } from "../shell/shell-event.ts";
import type { ShellAnalysis } from "../shell/shell-types.ts";
import { toJqJson } from "../state/state-io.ts";
import {
  ALLOW,
  gateConfig,
  preToolHostEvent,
  type GateModule,
  type GateModuleOptions,
} from "./gate-module.ts";

/** The gate file as parsed JSON; `undefined` when absent or not JSON, which never blocks. */
function readGate(file: string): unknown {
  if (!existsSync(file)) return undefined;
  try {
    const doc: unknown = JSON.parse(readFileSync(file, "utf8"));
    return doc;
  } catch {
    // Unreadable or malformed: bash's `jq` failed and the status read as "".
    return undefined;
  }
}

/** `$(jq -r '.<key> // "<fallback>"' <gate>)`: null, false and absent take the fallback. */
function field(doc: Record<string, unknown>, key: string, fallback: string): string {
  const value = doc[key];
  if (value === undefined || value === null || value === false) return fallback;
  const text = typeof value === "string" ? value : toJqJson(value, true);
  return text.replace(/\n+$/, "");
}

function gitDir(root: string, flag: string, env: HostEnv): string {
  const res = spawnSync("git", ["-C", root, "rev-parse", "--path-format=absolute", flag], {
    env: childEnv(env),
    encoding: "utf8",
  });
  const out = res.error === undefined && res.status === 0 ? res.stdout.trim() : "";
  return out.replace(/\/$/, "");
}

/** A linked worktree: quality state lives on the main checkout, so the gate never enforces here. */
function linkedWorktree(root: string, env: HostEnv): boolean {
  const dir = gitDir(root, "--git-dir", env);
  const common = gitDir(root, "--git-common-dir", env);
  return dir !== "" && common !== "" && dir !== common;
}

/**
 * Whether the line commits or pushes. A line the parser could not analyze (over
 * its size cap) may do either, so the gate applies: a failing gate must not be
 * shipped past by padding the command.
 */
function mayShip(analysis: ShellAnalysis): boolean {
  return analysis.unknown || isGitCommit(analysis) || isGitPush(analysis);
}

/** The lead sentence follows the mode: an advisory must not claim it blocked anything. */
const LEADS: Readonly<Record<Exclude<GateMode, "off">, string>> = {
  block: "BLOCKED: quality gate failing — fix the violations before committing or pushing.",
  ask: "The quality gate is failing. Commit/push anyway?",
  advise:
    "Heads up: the quality gate is failing (commit and push are not blocked at this setting).",
};

function decide(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  if (event.type !== "shell/pre" || envValue(ctx.env, "MY_CLAUDE_QUALITY") === "off") return ALLOW;
  if (!mayShip(shellAnalysisOf(event)) || !toolAvailable("git", ctx.env)) return ALLOW;
  const mode = gateMode(gateConfig(ctx, options), "qualityGate", {
    host: ctx.host,
    event: preToolHostEvent(event),
  });
  if (mode === "off") return ALLOW;
  const cwd = ctx.cwd ?? process.cwd();
  const root = gitToplevel(ctx.env, cwd) ?? cwd;
  const stateRoot = projectStateRoot({ root, env: ctx.env, host: ctx.host });
  if (stateRoot === undefined) return ALLOW;
  const doc = readGate(join(stateRoot, "quality-gate-status.json"));
  if (!isJsonObject(doc) || field(doc, "status", "") !== "failing") return ALLOW;
  if (linkedWorktree(root, ctx.env)) return ALLOW;
  const reason = field(doc, "reason", "Quality gate failing");
  const violations = field(doc, "violations", "");
  return gateDecision(mode, `${LEADS[mode]}\n${reason}\n${violations}`) ?? ALLOW;
}

/** The native `quality-gate` built-in PreToolUse module. */
export function qualityGateModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "quality-gate",
    run: (event, ctx) => Promise.resolve(decide(event, ctx, options)),
  };
}
