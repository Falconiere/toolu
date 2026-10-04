import type { ParsedArgs, Scope } from "../args/types";
import type { Marketplace } from "../catalog/types";
import { CliError, EXIT, UsageError, type ExitCode } from "../exit";
import type { InstallStep } from "../plugins/install";
import { anyFailed, reportList, reportPlanned, reportRemove, reportUpdate } from "../ui/report";
import { targetSpec } from "./entries";
import { opencodeInstall } from "./install";
import { writeAtomic } from "./jsonc";
import { opencodeList } from "./list";
import type { OpencodeScope } from "./paths";
import { opencodeRemove } from "./remove";
import { loadState, type OpencodePlan } from "./state";
import { opencodeUpdate } from "./update";

/** What the OpenCode verbs need beyond the parsed arguments. */
export interface OpencodeContext {
  readonly marketplace: Marketplace;
  readonly env: NodeJS.ProcessEnv;
  readonly cwd: string;
  readonly version: string;
  readonly write: (text: string) => void;
}

/** `--scope user` is OpenCode's global config, `project` the worktree root. */
function opencodeScope(scope: Scope | undefined): OpencodeScope | undefined {
  if (scope === undefined) return undefined;
  if (scope === "local")
    throw new UsageError("--scope local is Claude Code only; OpenCode has user and project");
  return scope === "user" ? "global" : "project";
}

/** Writes the plan's files unless this is a dry run. */
async function apply<Step>(plan: OpencodePlan<Step>, dryRun: boolean): Promise<readonly Step[]> {
  if (dryRun) return plan.steps;
  for (const write of plan.writes) {
    try {
      await writeAtomic(write.path, write.text);
    } catch (error) {
      throw new CliError(EXIT.failed, `cannot write ${write.path}: ${String(error)}`);
    }
  }
  return plan.steps;
}

export async function installOpencode(
  args: ParsedArgs,
  names: readonly string[],
  context: OpencodeContext,
): Promise<readonly InstallStep[]> {
  const state = await loadState(context.cwd, context.env);
  const request = {
    names,
    scope: opencodeScope(args.scope),
    target: targetSpec(context.env, context.version),
  };
  return apply(opencodeInstall(state, context.marketplace, request), args.dryRun);
}

/** list, remove and update on OpenCode. */
export async function dispatchOpencode(
  args: ParsedArgs & { readonly verb: "list" | "remove" | "update" },
  context: OpencodeContext,
): Promise<ExitCode> {
  const state = await loadState(context.cwd, context.env);
  const scope = opencodeScope(args.scope);
  if (args.verb === "list") {
    const { entries, header } = opencodeList(state, context.marketplace);
    context.write(
      args.json ? `${JSON.stringify(entries, null, 2)}\n` : header + reportList(entries),
    );
    return EXIT.ok;
  }
  if (args.verb === "remove") {
    const steps = await apply(
      opencodeRemove(state, context.marketplace, args.names, scope),
      args.dryRun,
    );
    context.write(args.dryRun ? reportPlanned(steps) : reportRemove(steps));
    return anyFailed(steps) ? EXIT.failed : EXIT.ok;
  }
  const target = targetSpec(context.env, context.version);
  const plan = opencodeUpdate(state, context.marketplace, args.names, scope, target);
  const steps = await apply(plan, args.dryRun);
  context.write(args.dryRun ? reportPlanned(steps) : reportUpdate(steps));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}
