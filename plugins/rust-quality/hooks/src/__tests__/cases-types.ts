/** The quality case shape and project preset shared with focused edge tests. */
import type { QualityCaseInput, QualityStep } from "@toolu/conformance/harness/quality-cases";

export type RsCase = QualityCaseInput;
export type Step = QualityStep;

export const RUST_PROJECT = { "Cargo.toml": '[package]\nname = "x"\nversion = "0.1.0"\n' };
