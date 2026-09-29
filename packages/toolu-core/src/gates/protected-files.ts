/**
 * protected-files (#260), the native port of `pre-tools/modules/protected-files.sh`.
 * An Edit/Write/MultiEdit path, or each file a Bash/Shell command writes, is
 * matched against `settings/protected-files.txt`; a match reaches the user as
 * `gates.protectedFiles.mode` says (ask by default, block where the host
 * cannot prompt). Shell writes come from `@toolu/core/shell/writes`, so a
 * redirect without a space, `cp -t`, and writes inside `bash -c` or `eval`
 * are seen (#283 items 1–3). A pathname pattern (`> .en[v]`) is every path it
 * expands to; a dynamic target (`> $HOME/.env`) is matched by its text.
 */
import { basename, join } from "node:path";
import { gateDecision, gateMode, guardrailWarning, type GateMode } from "../config/gate-mode.ts";
import { readList, SETTINGS_FILES } from "../config/settings.ts";
import type { Decision } from "../decision/decision.ts";
import { gitToplevel } from "../host/host-roots.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { shellAnalysisOf } from "../shell/shell-event.ts";
import { writeTargets } from "../shell/shell-writes.ts";
import { compileBashPattern, type BashPattern } from "./bash-pattern.ts";
import { expandPattern, repoRelative } from "./gate-paths.ts";
import {
  ALLOW,
  gateConfig,
  gateSettingsDir,
  inputString,
  preToolHostEvent,
  type GateModule,
  type GateModuleOptions,
} from "./gate-module.ts";

const EDIT_TOOLS = new Set(["Edit", "Write", "MultiEdit"]);

/** Each file a shell command writes, deduped in the order the command writes them. */
function shellCandidates(event: Extract<RegistryHookEvent, { type: "shell/pre" }>): string[] {
  const out = writeTargets(shellAnalysisOf(event)).flatMap((target) => {
    if (target.path !== null) return [target.path];
    if (target.pattern !== null) return expandPattern(target.pattern, event.cwd);
    return target.text === "" ? [] : [target.text];
  });
  return [...new Set(out.filter((path) => path !== ""))];
}

function candidates(event: RegistryHookEvent): string[] {
  if (event.type === "shell/pre") return shellCandidates(event);
  if (!EDIT_TOOLS.has(event.toolName)) return [];
  const path = inputString(event, "file_path");
  return path === "" ? [] : [path];
}

type Rule = { pattern: string; tests: BashPattern[]; byName: BashPattern | undefined };

/** A line is tried as written, then anchored anywhere (`**\/`), then against the basename. */
function rule(pattern: string): Rule {
  const tests = [compileBashPattern(pattern)];
  if (!pattern.startsWith("**/")) tests.push(compileBashPattern(`**/${pattern}`));
  const byName = pattern.includes("/") ? undefined : compileBashPattern(pattern);
  return { pattern, tests, byName };
}

function firstMatch(rules: readonly Rule[], rel: string): string | undefined {
  return rules.find((r) => r.tests.some((t) => t(rel)) || r.byName?.(basename(rel)) === true)
    ?.pattern;
}

/** Why a path is guarded, by kind: the first `case` arm of the bash module that matches. */
const DETAILS: readonly [BashPattern, string][] = [
  [
    compileBashPattern("@(*.env.example|*.env.template|*.env.sample)"),
    "This is an example/template env file. It is committed on purpose, so it should carry placeholders and never live values — it is guarded because a real credential pasted here is a credential published to the repo.",
  ],
  [
    compileBashPattern("@(.env|.env.*|*secrets*)"),
    "This is a secrets file. Approving lets an agent read or rewrite live credentials, and anything it writes here can leak into logs, commits, or a diff you push.",
  ],
  [
    compileBashPattern("@(.git/*|*/.git/*)"),
    "This is git's internal state. Approving lets an agent rewrite refs, hooks, or config — including hooks that run on your machine at every commit.",
  ],
  [
    compileBashPattern("@(*hooks/*|*skills/*)"),
    "This is toolu's own enforcement code — the hooks that run every other gate. Approving lets an agent edit the thing that is supposed to be watching it, which is how a guardrail gets quietly switched off.",
  ],
];
const DEFAULT_DETAIL =
  "This path is listed in settings/protected-files.txt because edits to it are hard to notice and expensive to get wrong.";

function detailFor(rel: string): string {
  return DETAILS.find(([test]) => test(rel))?.[1] ?? DEFAULT_DETAIL;
}

type Hit = { candidate: string; rel: string; matched: string; shell: boolean };

function reason(mode: GateMode, hit: Hit): string {
  const { candidate, matched } = hit;
  const detail = detailFor(hit.rel);
  const headline = hit.shell
    ? `This command would WRITE to ${candidate}, a protected path (matches "${matched}").`
    : `Claude is trying to edit ${candidate}, a protected path (matches "${matched}").`;
  if (mode === "ask") return guardrailWarning(headline, detail);
  if (mode === "advise") {
    return `Protected path ${candidate} (matches "${matched}"). ${detail} The write was NOT stopped — gates.protectedFiles.mode is 'advise'.`;
  }
  return `${headline} ${detail} Blocked by gates.protectedFiles.mode='block' (see plugins/toolu/hooks/docs/gates.md).`;
}

function findHit(
  paths: readonly string[],
  rules: readonly Rule[],
  ctx: RegistryContext,
  shell: boolean,
): Hit | undefined {
  let root: string | undefined;
  for (const candidate of paths) {
    // Only an absolute path can carry the repo root; skip the git spawn otherwise.
    if (candidate.startsWith("/")) root ??= gitToplevel(ctx.env, process.cwd()) ?? "";
    const rel = repoRelative(candidate, root);
    const matched = firstMatch(rules, rel);
    if (matched !== undefined) return { candidate, rel, matched, shell };
  }
  return undefined;
}

function decide(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  // The list first, as bash did: no list means no command is parsed.
  const dir = gateSettingsDir(ctx, options);
  if (dir === undefined) return ALLOW;
  const rules = readList(join(dir, SETTINGS_FILES.protectedFiles)).map(rule);
  if (rules.length === 0) return ALLOW;
  const paths = candidates(event);
  if (paths.length === 0) return ALLOW;
  const hit = findHit(paths, rules, ctx, event.type === "shell/pre");
  if (hit === undefined) return ALLOW;
  const mode = gateMode(gateConfig(ctx, options), "protectedFiles", {
    host: ctx.host,
    event: preToolHostEvent(event),
  });
  return gateDecision(mode, reason(mode, hit)) ?? ALLOW;
}

/** The protected-files built-in module. */
export function protectedFilesModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "protected-files",
    run: (event, ctx) => Promise.resolve(decide(event, ctx, options)),
  };
}
