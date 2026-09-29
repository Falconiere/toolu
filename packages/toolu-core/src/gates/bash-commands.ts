/**
 * bash-commands (#261): the Bash/Shell deny and allow lists, a port of
 * `pre-tools/modules/bash-commands.sh` on `@toolu/core/shell` (#284). Each
 * rule is matched against every simple command the line runs: after `&&`, `;`
 * and pipes, in subshells, behind env prefixes and wrappers such as `sudo`, and
 * inside `bash -c`/`eval` (#283 items 3 and 4). An allow rule overrides a deny
 * only on the command the deny matched. A line that cannot be analyzed is a hit.
 * No `python3` is needed.
 */
import { gateDecision, gateMode, guardrailWarning, type GateMode } from "../config/gate-mode.ts";
import { bashAllowlist, bashDenylist } from "../config/settings.ts";
import type { Decision } from "../decision/decision.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { shellAnalysisOf } from "../shell/shell-event.ts";
import { matchesRule } from "../shell/shell-rules.ts";
import type { ShellAnalysis, ShellCommand } from "../shell/shell-types.ts";
import {
  ALLOW,
  gateConfig,
  gateSettingsDir,
  preToolHostEvent,
  type GateModule,
  type GateModuleOptions,
} from "./gate-module.ts";

export type BashLists = { readonly allow: readonly string[]; readonly deny: readonly string[] };

/**
 * `unknown`: the line could not be analyzed (over the parser's size cap, or no
 * command could be read), so no rule can be checked. A guardrail treats that as
 * a hit, never as "nothing runs".
 */
export type BashCommandsVerdict =
  | { kind: "allow" }
  | { kind: "deny"; rule: string }
  | { kind: "unknown"; why: string };

/**
 * A rule without a space (`biome`) is a substring of the command's source text,
 * which holds its arguments and redirect targets but no heredoc body. Any other
 * rule (`node -e`) is argv-aware.
 */
function matches(command: ShellCommand, rule: string): boolean {
  if (!rule.includes(" ")) return command.text.includes(rule);
  return matchesRule(command, rule);
}

/**
 * `bash_commands_decide`: the first deny rule, in file order, that some command
 * matches while no allow rule matches that same command.
 */
export function bashCommandsDecide(analysis: ShellAnalysis, lists: BashLists): BashCommandsVerdict {
  if (analysis.unknown) {
    return { kind: "unknown", why: analysis.errors[0]?.message ?? "no command could be read" };
  }
  const deniedBy = (rule: string) =>
    analysis.commands.some(
      (command) => matches(command, rule) && !lists.allow.some((allow) => matches(command, allow)),
    );
  const rule = lists.deny.find(deniedBy);
  return rule === undefined ? { kind: "allow" } : { kind: "deny", rule };
}

const WHY_GUARDED =
  "Rules in settings/bash-denylist.txt cover commands that execute arbitrary code from a string (node -e, bun -e) or that this project has ruled out.";

function ruleReason(mode: GateMode, rule: string): string {
  if (mode === "ask") {
    return guardrailWarning(
      `Claude wants to run a command matching the deny rule "${rule}".`,
      `${WHY_GUARDED} The command runs with your full shell privileges if you approve.`,
    );
  }
  if (mode === "advise") {
    return `Command matches deny rule "${rule}" (plugins/toolu/settings/bash-denylist.txt). The command was not stopped — gates.bashCommands.mode is 'advise'.`;
  }
  return `Command blocked by deny rule: ${rule}`;
}

function unknownReason(mode: GateMode, why: string): string {
  if (mode === "ask") {
    return guardrailWarning(
      `Claude wants to run a command toolu could not analyze (${why}).`,
      `${WHY_GUARDED} A command that cannot be parsed cannot be checked against them. It runs with your full shell privileges if you approve.`,
    );
  }
  if (mode === "advise") {
    return `Command could not be analyzed (${why}), so plugins/toolu/settings/bash-denylist.txt was not checked. The command was not stopped — gates.bashCommands.mode is 'advise'.`;
  }
  return `Command blocked: it could not be analyzed against the deny rules (${why})`;
}

function decide(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  if (event.type !== "shell/pre") return ALLOW;
  const dir = gateSettingsDir(ctx, options);
  if (dir === undefined) return ALLOW;
  const deny = bashDenylist(dir);
  if (deny.length === 0) return ALLOW;
  const verdict = bashCommandsDecide(shellAnalysisOf(event), { allow: bashAllowlist(dir), deny });
  if (verdict.kind === "allow") return ALLOW;
  const mode = gateMode(gateConfig(ctx, options), "bashCommands", {
    host: ctx.host,
    event: preToolHostEvent(event),
  });
  const text =
    verdict.kind === "deny" ? ruleReason(mode, verdict.rule) : unknownReason(mode, verdict.why);
  return gateDecision(mode, text) ?? ALLOW;
}

/** The native `bash-commands` built-in PreToolUse module. */
export function bashCommandsModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "bash-commands",
    run: (event, ctx) => Promise.resolve(decide(event, ctx, options)),
  };
}
