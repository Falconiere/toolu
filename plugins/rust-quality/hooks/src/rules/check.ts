/**
 * Every rust-quality rule over one file (#267), in the numeric order of the
 * bash fragments they replace, so violations are reported in the same order.
 * The docs advisory is shown only when nothing blocks.
 */
import type { QualityFindings } from "@toolu/core/quality";
import { errorHandling, noMocks } from "./ast-rules.ts";
import { docs } from "./docs.ts";
import { isRustTest, testLayout } from "./layout-rules.ts";
import { suppression, unsafeCode } from "./line-rules.ts";
import type { RsFile } from "./rs-file.ts";
import { fileTooLong, functionTooLong, implTooLong } from "./size-rules.ts";

/** The findings for `f`: blocking errors, then the docs advisories. */
export function checkRsFile(f: RsFile): QualityFindings {
  const isTest = isRustTest(f);
  const errors = [
    fileTooLong(f),
    ...testLayout(f, isTest),
    suppression(f),
    unsafeCode(f),
    functionTooLong(f, isTest),
    implTooLong(f, isTest),
    ...errorHandling(f, isTest),
    ...noMocks(f),
  ].filter((error) => error !== undefined);
  return { errors, advisories: docs(f, isTest) };
}
