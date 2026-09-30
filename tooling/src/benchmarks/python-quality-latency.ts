/**
 * python-quality PostToolUse latency (#266, AC-9): the bash module at the
 * golden's base commit against the TypeScript module, both behind the
 * post-tools bundle. See `quality-latency.ts`.
 *
 * Usage: bun run tooling/src/benchmarks/python-quality-latency.ts [--runs N] [--assert]
 */
import { PY_CASES } from "../../../plugins/python-quality/hooks/src/__tests__/cases.ts";
import {
  BASH_BASE,
  dispatchStep,
  extractBaseRegister,
  setupCase,
} from "../../../plugins/python-quality/hooks/src/__tests__/golden-harness.ts";
import { qualityLatency } from "./quality-latency.ts";

const SLICE = [
  "assembled: two violations in fragment order",
  "suppression: bare except:",
  "no-mocks: from unittest.mock import MagicMock, patch",
  "size: def at the default fn-length limit",
  "assembled: clean file",
];

process.exitCode = await qualityLatency(
  {
    label: "python-quality-latency",
    cases: PY_CASES,
    slice: SLICE,
    base: BASH_BASE,
    extractBaseRegister,
    setupCase,
    dispatchStep,
  },
  process.argv.slice(2),
);
