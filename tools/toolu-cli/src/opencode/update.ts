import { installOrder } from "../catalog/order";
import type { Marketplace } from "../catalog/types";
import { CliError, EXIT } from "../exit";
import type { UpdateStep } from "../plugins/update";
import { PACKAGE, tooluEntries } from "./entries";
import { editJsonc, type ConfigFile } from "./jsonc";
import type { OpencodeScope } from "./paths";
import { packageScopes } from "./remove";
import { filesOf, type OpencodePlan, type OpencodeState, type PlannedWrite } from "./state";

function updateFile(file: ConfigFile, target: string): OpencodePlan<UpdateStep> {
  let text = file.text ?? "";
  const steps = tooluEntries(file).map((entry): UpdateStep => {
    if (entry.spec === target) {
      return {
        name: PACKAGE,
        outcome: "current",
        detail: `current at ${target} in ${file.path}`,
        argv: [],
      };
    }
    text = editJsonc(
      text,
      entry.tuple ? ["plugin", entry.index, 0] : ["plugin", entry.index],
      target,
    );
    const change = `${entry.spec} -> ${target} in ${file.path}`;
    return {
      name: PACKAGE,
      outcome: "updated",
      detail: `updated ${change}`,
      argv: [],
      plan: `update ${change}`,
    };
  });
  const changed = steps.some((step) => step.outcome === "updated");
  const writes: PlannedWrite[] = changed ? [{ path: file.path, text }] : [];
  return { steps, writes };
}

/**
 * Rewrites every toolu entry in scope to the target spec, keeping tuple options.
 * Names are checked against the catalog but cannot narrow the update: one
 * package carries every plugin.
 */
export function opencodeUpdate(
  state: OpencodeState,
  marketplace: Marketplace,
  names: readonly string[],
  scope: OpencodeScope | undefined,
  target: string,
): OpencodePlan<UpdateStep> {
  if (names.length > 0) installOrder(marketplace, names);
  const files = packageScopes(state, scope)
    .flatMap((s) => filesOf(state, s))
    .filter((file) => tooluEntries(file).length > 0);
  if (files.length === 0) {
    throw new CliError(
      EXIT.failed,
      `${PACKAGE} is not configured in OpenCode${scope === undefined ? "" : ` (${scope} scope)`}, so there is nothing to update`,
    );
  }
  const plans = files.map((file) => updateFile(file, target));
  return { steps: plans.flatMap((p) => p.steps), writes: plans.flatMap((p) => p.writes) };
}
