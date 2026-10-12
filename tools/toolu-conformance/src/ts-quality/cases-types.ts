/** The quality case shape and project presets shared with focused edge tests. */
import type { QualityCaseInput, QualityStep } from "@toolu/conformance/harness/quality-cases";

export type TsCase = QualityCaseInput;
export type Step = QualityStep;

export const TS_PROJECT = {
  "tsconfig.json": "{}\n",
  "package.json": '{"name":"x"}\n',
  "bun.lock": "",
};

export const ALIAS_PROJECT = {
  ...TS_PROJECT,
  "tsconfig.json": '{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}\n',
};
