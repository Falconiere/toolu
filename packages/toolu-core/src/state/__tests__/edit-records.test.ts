/** The shared edit-records golden (#415): kind, records and printed text for every payload. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";
import { formatEditRecords, normalizeEditRecords } from "../edit-records.ts";
import { EditRecordSchema } from "../state-schema.ts";

const CaseSchema = z.strictObject({
  name: z.string(),
  tool: z.string(),
  payload: z.json(),
  expect: z.union([
    z.strictObject({
      kind: z.literal("records"),
      records: z.array(EditRecordSchema),
      text: z.string(),
    }),
    z.strictObject({ kind: z.enum(["not-edit", "malformed"]) }),
  ]),
});
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/state/edit-records.json"),
).map((raw) => CaseSchema.parse(raw));

test("the golden holds its forty-two payloads", () => {
  expect(cases).toHaveLength(42);
});

for (const c of cases) {
  test(c.name, () => {
    const result = normalizeEditRecords(c.payload, c.tool);
    const got =
      result.kind === "records"
        ? { kind: result.kind, records: result.records, text: formatEditRecords(result.records) }
        : { kind: result.kind };
    expect<unknown>(got).toEqual(c.expect);
  });
}
