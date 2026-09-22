/** Per-suite outcome for the conformance matrix (#212). */
export type SuiteOutcome =
  | { status: "pass" }
  | { status: "fail"; message: string }
  | { status: "skip"; message: string };

export type SuiteResult = {
  id: string;
  outcome: SuiteOutcome;
};

export type ConformanceResult = { pass: true } | { pass: false; message: string };

export type SuiteDefinition = {
  id: string;
  run: () => Promise<SuiteOutcome>;
};
