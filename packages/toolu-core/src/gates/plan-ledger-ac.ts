/**
 * The plan-ledger push gate's AC coverage (#262): report-only `ac_coverage`
 * telemetry for the plan's spec, and the opt-in `planLedger.blockOnUncoveredAcs`
 * push stop. A port of the AC half of `pre-tools/modules/plan-ledger.sh`.
 */
import { join, resolve } from "node:path";
import type { LoadedConfig } from "../config/config-load.ts";
import { flagTrue } from "../config/config-read.ts";
import { alt, get, type Json } from "../ledger/ledger-jq.ts";
import { acCoverage } from "../ledger/ledger-model.ts";
import { docField, isFile, isSpecless } from "../ledger/ledger-parse.ts";
import { rawOr } from "../ledger/verdict-gates.ts";
import type { RegistryContext } from "../registry/registry-types.ts";
import { telemetryAppend } from "../state/telemetry.ts";

/** bash `[[ -f $p ]] || { [[ -f $root/$p ]] && p=$root/$p; }`, relative paths read from `cwd`. */
function locate(cwd: string, root: string, path: string): string {
  const here = resolve(cwd, path);
  if (isFile(here)) return here;
  const there = join(root, path);
  return root !== "" && isFile(there) ? there : here;
}

const AC_LINE = /^ {2}AC-/;
const UNCOVERED = /^ {2}AC-[^:]+: UNCOVERED/;

/**
 * Record the plan's AC coverage at `cur` under `root`. Returns the uncovered
 * lines (leading indent dropped) when `blockOnUncoveredAcs` turns them into a
 * push stop, else undefined.
 */
export function acBlockers(
  ledger: Json,
  cur: string,
  root: string,
  ctx: RegistryContext,
  config: LoadedConfig,
): string | undefined {
  const planField = rawOr(() => alt(get(ledger, "plan_doc"), ""));
  if (planField === "") return undefined;
  const cwd = ctx.cwd ?? process.cwd();
  const plan = locate(cwd, root, planField);
  if (!isFile(plan)) return undefined;
  const specField = docField(plan, "Spec");
  const spec = isSpecless(specField) ? specField : locate(cwd, root, specField);
  const report = acCoverage(ledger, cur, spec).stdout.replace(/\n+$/, "");
  if (report === "") return undefined;
  const lines = report.split("\n");
  const uncovered = lines.filter((line) => UNCOVERED.test(line));
  const covered = lines.filter((line) => AC_LINE.test(line)).length - uncovered.length;
  const options = { env: ctx.env, host: ctx.host };
  telemetryAppend(root, "ac_coverage", { covered, uncovered: uncovered.length }, options);
  if (uncovered.length === 0 || !flagTrue(config, "planLedger", "blockOnUncoveredAcs")) {
    return undefined;
  }
  return uncovered.map((line) => line.slice(2)).join("\n");
}
