/**
 * Every python-quality rule over one file (#266), in the numeric order of the
 * bash fragments they replace, so violations are reported in the same order.
 * The docs advisory always runs; it is shown only when nothing blocks.
 */
import { basename } from "node:path";
import type { QualityFindings } from "@toolu/core/quality";
import { docs } from "./docs.ts";
import { testLayout } from "./layout-rules.ts";
import { noMocks } from "./no-mocks.ts";
import { isPythonTestName, type PyFile } from "./py-file.ts";
import { fileTooLong, functionTooLong } from "./size-rules.ts";
import { suppression } from "./suppression.ts";

/** The findings for `f`: blocking errors, then the docs advisory. */
export function checkPyFile(f: PyFile): QualityFindings {
  const isTest = isPythonTestName(basename(f.file.path));
  const errors = [
    fileTooLong(f),
    ...testLayout(f),
    suppression(f),
    functionTooLong(f),
    ...noMocks(f, isTest),
  ].filter((error) => error !== undefined);
  return { errors, advisories: [docs(f)] };
}
