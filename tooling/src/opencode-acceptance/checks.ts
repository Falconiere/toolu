/**
 * The shape of one OpenCode acceptance check (#362): what it proves, for which
 * catalog plugins, and how. `evidence` keeps three things apart:
 *
 * - `execution`: `actual-host` when the pinned `opencode` binary ran the
 *   session, `in-process` when hooks were called directly;
 * - `model`: always `scripted-loopback`, a fixture that only scripts replies;
 * - `service`: whether an external service was absent (`none`), stood in for
 *   by a loopback fixture or bare git remote (`fixture`), or really called
 *   (`live`). Live calls are reported apart and never decide acceptance.
 */
import { ContractError } from "../opencode-host/schema.ts";
import type { EntryContext } from "../opencode-host/scenarios-entry.ts";

export type Service = "none" | "fixture" | "live";
type Evidence = {
  execution: "actual-host" | "in-process";
  model: "scripted-loopback";
  service: Service;
};
type Observed = Record<string, unknown>;
export type CheckOutcome = { pass: boolean; observed: Observed };

/** What every check receives: the pinned host, the shared run cache and the packed tarball. */
export type AcceptanceContext = EntryContext;

export type AcceptanceCheck = {
  id: string;
  family: string;
  /** The catalog plugins this check proves; `"all"` for whole-catalog checks, `[]` for the host contract. */
  plugins: readonly string[] | "all";
  evidence: Evidence;
  run: (ctx: AcceptanceContext) => Promise<CheckOutcome>;
};

/** Evidence of a session the pinned host ran against the scripted model. */
export function hostEvidence(service: Service = "none"): Evidence {
  return { execution: "actual-host", model: "scripted-loopback", service };
}

type ScenarioLike = {
  id: string;
  run: (ctx: AcceptanceContext) => Promise<{ pass: boolean; observed: Observed }>;
};

/** Checks of one family, one per scenario of an existing live scenario array. */
export function scenarioChecks(
  family: string,
  plugins: readonly string[] | "all",
  service: Service,
  scenarios: readonly ScenarioLike[],
): AcceptanceCheck[] {
  return scenarios.map((scenario) => ({
    id: scenario.id,
    family,
    plugins,
    evidence: hostEvidence(service),
    run: async (ctx) => {
      const { pass, observed } = await scenario.run(ctx);
      return { pass, observed };
    },
  }));
}

/**
 * `fn` over `items` one at a time, in order. Live sessions share the host's
 * caches and must not race, and a report lists results in run order.
 */
export async function inSequence<T, R>(
  items: readonly T[],
  fn: (item: T) => Promise<R>,
): Promise<R[]> {
  const [first, ...rest] = items;
  if (first === undefined) return [];
  const head = await fn(first);
  return [head, ...(await inSequence(rest, fn))];
}

/** The checks named in `ids`, in registry order; an unknown id is an error, never an empty run. */
export function selectChecks(
  checks: readonly AcceptanceCheck[],
  ids: readonly string[],
): AcceptanceCheck[] {
  const unknown = ids.filter((id) => !checks.some((check) => check.id === id));
  if (unknown.length > 0)
    throw new ContractError(`unknown acceptance check: ${unknown.join(", ")}`);
  return ids.length === 0 ? [...checks] : checks.filter((check) => ids.includes(check.id));
}

/** Whether `check` is an actual-host check dedicated to `plugin`. */
function dedicated(check: AcceptanceCheck, plugin: string): boolean {
  return (
    check.evidence.execution === "actual-host" &&
    check.plugins !== "all" &&
    check.plugins.includes(plugin)
  );
}

/**
 * Plugin → the passing actual-host checks dedicated to it. A whole-catalog
 * (`"all"`) check proves loading, not a plugin's own behavior, so it never
 * counts toward a plugin's coverage.
 */
export function coverage(
  results: readonly { check: AcceptanceCheck; pass: boolean }[],
  catalog: readonly string[],
): Record<string, string[]> {
  return Object.fromEntries(
    catalog.map((name) => [
      name,
      results
        .filter(({ check, pass }) => pass && dedicated(check, name))
        .map(({ check }) => check.id),
    ]),
  );
}
