/** The quality case shape and project preset shared with focused edge tests. */
import type { QualityCaseInput, QualityStep } from "@toolu/conformance/harness/quality-cases";

export type PyCase = QualityCaseInput;
export type Step = QualityStep;

export const PY_PROJECT = { "pyproject.toml": '[project]\nname = "x"\nversion = "0.1.0"\n' };
