import { runBootstrapReadinessSuite } from "./suites/bootstrap-readiness.ts";
import { runProtectedFilesSuite } from "./suites/protected-files.ts";
import { runSpacesCwdSuite } from "./suites/spaces-cwd.ts";
import { runSurfaceDriftSuite } from "./suites/surface-drift.ts";
import type { ConformanceResult, SuiteDefinition, SuiteOutcome, SuiteResult } from "./types.ts";

export const CONFORMANCE_SUITES: SuiteDefinition[] = [
  { id: "protected-files", run: runProtectedFilesSuite },
  { id: "bootstrap-readiness", run: runBootstrapReadinessSuite },
  { id: "surface-drift", run: runSurfaceDriftSuite },
  { id: "spaces-cwd", run: runSpacesCwdSuite },
];

export function formatSuiteLine(id: string, outcome: SuiteOutcome): string {
  if (outcome.status === "pass") {
    return `toolu-conformance: ${id} pass`;
  }
  return `toolu-conformance: ${id} fail (${outcome.message})`;
}

async function runSuiteDefinition(suite: SuiteDefinition): Promise<SuiteResult> {
  let outcome: SuiteOutcome;
  try {
    outcome = await suite.run();
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    outcome = { status: "fail", message };
  }
  return { id: suite.id, outcome };
}

async function runSuitesSequential(
  suites: SuiteDefinition[],
  index: number,
  acc: SuiteResult[],
): Promise<SuiteResult[]> {
  const suite = suites[index];
  if (suite === undefined) {
    return acc;
  }
  const next = await runSuiteDefinition(suite);
  return runSuitesSequential(suites, index + 1, [...acc, next]);
}

/** Run all fixture suites; fails closed on any suite failure (#212 AC-1). */
export async function runConformanceMatrix(
  suites: SuiteDefinition[] = CONFORMANCE_SUITES,
): Promise<{ results: SuiteResult[]; pass: boolean }> {
  const results = await runSuitesSequential(suites, 0, []);
  const pass = results.every((r) => r.outcome.status !== "fail");
  return { results, pass };
}

/** Back-compat: protected-files only (#210). */
export async function runProtectedFilesConformance(): Promise<ConformanceResult> {
  const outcome = await runProtectedFilesSuite();
  if (outcome.status === "pass") {
    return { pass: true };
  }
  return { pass: false, message: outcome.message };
}
