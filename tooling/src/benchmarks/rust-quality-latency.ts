/**
 * rust-quality PostToolUse latency (#267, AC-9): the bash module at the
 * golden's base commit against the TypeScript module, through the same
 * post-tools bundle, on a slice of the golden cases.
 *
 * Usage: bun run tooling/src/benchmarks/rust-quality-latency.ts [--runs N] [--assert]
 */
import { RS_CASES } from "../../../plugins/rust-quality/hooks/src/__tests__/cases.ts";
import {
  BASH_BASE,
  dispatchStep,
  extractBaseRegister,
  setupCase,
} from "../../../plugins/rust-quality/hooks/src/__tests__/golden-harness.ts";
import { qualityLatency } from "./quality-latency.ts";

process.exitCode = await qualityLatency(
  {
    label: "rust-quality-latency",
    cases: RS_CASES,
    slice: [
      "assembled: two violations in rule order",
      "error-handling: .unwrap() in src/ is flagged",
      "no-mocks: a bare #[automock] in src/ fails the gate",
      "size: long method inside an impl IS flagged",
      "assembled: clean file writes no failure",
    ],
    base: BASH_BASE,
    extractBaseRegister,
    setupCase,
    dispatchStep,
  },
  process.argv.slice(2),
);
