/** Public state package surface from the shared Rust parity record. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import * as state from "@toolu/core/state";
import { z } from "zod";

const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("package"),
  exports: z.array(z.string()),
  gateFileVersion: z.number().int(),
  validGateFile: z.json(),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/state/cases.json"))
  .filter((raw) => raw.kind === "package")
  .map((raw) => CaseSchema.parse(raw));
for (const c of cases) {
  test(c.name, () => {
    for (const name of c.exports) {
      expect(typeof Object.entries(state).find(([key]) => key === name)?.[1]).toBe("function");
    }
    expect(state.GATE_FILE_VERSION).toBe(c.gateFileVersion);
    expect(state.GateFileSchema.safeParse(c.validGateFile).success).toBe(true);
  });
}
