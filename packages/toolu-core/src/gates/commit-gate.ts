/**
 * commit-gate (#261): the Conventional Commits prefix check and the
 * before-commit reminder, a port of `pre-tools/modules/commit-gate.sh`. The
 * commit and its message come from the parsed argv (`@toolu/core/shell`), so
 * `-m "$(cat <<'EOF' … EOF)"`, `-am`, `-m'…'`, `--message=` and a commit behind
 * `sudo`, `/usr/bin/git` or `bash -c` are all seen (#283 items 5 and 8).
 */
import { gateDecision, gateMode } from "../config/gate-mode.ts";
import { commitPrefixes } from "../config/settings.ts";
import type { Decision } from "../decision/decision.ts";
import { baseBranch } from "../detect/detect-branch.ts";
import { isGitCommit } from "../detect/detect-git.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { shellAnalysisOf } from "../shell/shell-event.ts";
import { commitMessages, gitInvocation } from "../shell/shell-git.ts";
import type { ShellAnalysis } from "../shell/shell-types.ts";
import {
  ALLOW,
  gateConfig,
  gateSettingsDir,
  preToolHostEvent,
  type GateModule,
  type GateModuleOptions,
} from "./gate-module.ts";

/** The type of a Conventional Commits subject: `^([a-z]+)(\(.*\))?:` on the message's first line. */
export function commitPrefix(message: string): string | undefined {
  const subject = message.split("\n", 1)[0] ?? "";
  return /^([a-z]+)(\(.*\))?:/s.exec(subject)?.[1];
}

/**
 * The first commit whose subject (its first `-m`, the paragraph git records as
 * the subject) has a type outside `prefixes`. A dynamic message is not checked,
 * as bash did not check a message it could not extract.
 */
function unknownPrefix(analysis: ShellAnalysis, prefixes: readonly string[]): string | undefined {
  for (const command of analysis.commands) {
    const git = gitInvocation(command);
    if (git?.subcommand !== "commit") continue;
    const subject = commitMessages(git)[0];
    const prefix = typeof subject === "string" ? commitPrefix(subject) : undefined;
    if (prefix !== undefined && !prefixes.includes(prefix)) return prefix;
  }
  return undefined;
}

/**
 * Whether the line commits. A line the parser could not analyze (over its size
 * cap) gets the reminder when it names `git` and `commit`; its message cannot
 * be read, so it is not prefix-checked.
 */
function commits(analysis: ShellAnalysis): boolean {
  if (!analysis.unknown) return isGitCommit(analysis);
  return /\bgit\b/.test(analysis.source) && /\bcommit\b/.test(analysis.source);
}

function reminder(base: string): string {
  return `BEFORE COMMITTING:\n1. Verify diff covers only expected scope (git diff --stat against ${base})\n2. Save memory of significant decisions before committing.\nSkip only if already done this task.`;
}

function decide(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  if (event.type !== "shell/pre" || event.toolName !== "Bash") return ALLOW;
  const analysis = shellAnalysisOf(event);
  if (!commits(analysis)) return ALLOW;
  const mode = gateMode(gateConfig(ctx, options), "commitGate", {
    host: ctx.host,
    event: preToolHostEvent(event),
  });
  if (mode === "off") return ALLOW;
  const base = baseBranch(undefined, ctx.env, ctx.cwd);
  const dir = gateSettingsDir(ctx, options);
  const prefixes = dir === undefined ? [] : commitPrefixes(dir);
  const bad = prefixes.length === 0 ? undefined : unknownPrefix(analysis, prefixes);
  if (bad === undefined) return { kind: "advisory", message: reminder(base) };
  const reason = `Unknown Conventional Commits prefix: "${bad}". Allowed prefixes are in settings/commit-prefixes.txt. Base branch: ${base}`;
  return gateDecision(mode, reason) ?? ALLOW;
}

/** The native `commit-gate` built-in PreToolUse module. */
export function commitGateModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "commit-gate",
    run: (event, ctx) => Promise.resolve(decide(event, ctx, options)),
  };
}
