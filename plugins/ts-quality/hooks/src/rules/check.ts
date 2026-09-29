/**
 * Every ts-quality rule over one file (#265), in the numeric order of the bash
 * fragments they replace, so violations are reported in the same order.
 * Duplication runs only when nothing blocks (pre-existing duplication is not
 * the editor's fault); docs and handler advisories always run.
 */
import type { QualityFindings } from "@toolu/core/quality";
import { docs, duplication, unhandledAwait } from "./advisories.ts";
import { errorHandling, mockDoubles } from "./ast-rules.ts";
import { parentImport, testPlacement } from "./layout-rules.ts";
import {
  catchToast,
  componentFileName,
  confirmAlert,
  consoleLog,
  factories,
  manualTypeGuard,
  mutableProps,
  rawRadixImport,
  reactHooks,
  suppressionComment,
  typeAssertion,
} from "./line-rules.ts";
import { fileTooLong, functionTooLong } from "./size-rules.ts";
import type { TsFile } from "./ts-file.ts";
import { duplicateTypes, throwLiteral } from "./type-rules.ts";

type Rule = (f: TsFile) => string | undefined | readonly string[];

const RULES: readonly Rule[] = [
  parentImport,
  typeAssertion,
  testPlacement,
  fileTooLong,
  functionTooLong,
  reactHooks,
  factories,
  manualTypeGuard,
  duplicateTypes,
  componentFileName,
  consoleLog,
  suppressionComment,
  confirmAlert,
  rawRadixImport,
  mutableProps,
  catchToast,
  errorHandling,
  throwLiteral,
  mockDoubles,
];

/** The findings for `f`: blocking errors, then the three advisories. */
export function checkTsFile(f: TsFile): QualityFindings {
  const errors = RULES.flatMap((rule) => rule(f) ?? []);
  const duplicated = errors.length === 0 ? duplication(f) : "";
  return { errors, advisories: [duplicated, docs(f), unhandledAwait(f)] };
}
