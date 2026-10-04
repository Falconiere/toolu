/**
 * Per-suite outcome for the conformance matrix (#212). There is no skip: a
 * suite that cannot run fails (#362). Live OpenCode acceptance is
 * `bun run test:opencode`, not a conformance suite.
 */
export type SuiteOutcome = { status: "pass" } | { status: "fail"; message: string };

export type SuiteResult = {
  id: string;
  outcome: SuiteOutcome;
};

export type ConformanceResult = { pass: true } | { pass: false; message: string };

export type SuiteDefinition = {
  id: string;
  run: () => Promise<SuiteOutcome>;
};
