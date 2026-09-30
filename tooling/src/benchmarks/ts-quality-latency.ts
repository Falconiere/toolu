/**
 * ts-quality PostToolUse latency (#265, AC-8): the bash module at the golden's
 * base commit against the TypeScript module, both behind the post-tools
 * bundle. See `quality-latency.ts`.
 *
 * Usage: bun run tooling/src/benchmarks/ts-quality-latency.ts [--runs N] [--assert]
 */
import { TS_CASES } from "../../../plugins/ts-quality/hooks/src/__tests__/cases.ts";
import {
  BASH_BASE,
  dispatchStep,
  extractBaseRegister,
  setupCase,
} from "../../../plugins/ts-quality/hooks/src/__tests__/golden-harness.ts";
import { qualityLatency } from "./quality-latency.ts";

const SLICE = [
  "assembled: three violations in fragment order",
  "errors: empty catch block",
  "no-mocks: vi.mock",
  "size: long class method",
  "assembled: clean file",
];

process.exitCode = await qualityLatency(
  {
    label: "ts-quality-latency",
    cases: TS_CASES,
    slice: SLICE,
    base: BASH_BASE,
    extractBaseRegister,
    setupCase,
    dispatchStep,
  },
  process.argv.slice(2),
);
