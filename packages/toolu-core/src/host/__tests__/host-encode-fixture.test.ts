/**
 * The shared encode fixture (#413): `encodeDecision` reproduces every case of
 * `fixtures/host/encode.json` byte for byte. The Rust encoder
 * (`crates/core/protocol/tests/encode_fixture.rs`) checks the same cases.
 */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";
import { DecisionSchema, type Decision } from "../../decision/decision.ts";
import { degradeAsk, encodeDecision } from "../host-encode.ts";
import { HOST_EVENTS } from "../host-events.ts";
import { HOST_NAMES } from "../host-name.ts";

const ExpectSchema = z.union([
  z.strictObject({ stdout: z.string() }),
  z.strictObject({ callback: z.string() }),
  z.strictObject({ error: z.string() }),
]);
const CaseSchema = z.strictObject({
  name: z.string().min(1),
  host: z.enum(HOST_NAMES),
  event: z.enum(HOST_EVENTS),
  decision: DecisionSchema,
  gateClass: z.enum(["guardrail", "judgement"]).optional(),
  expect: ExpectSchema,
});
type Case = z.infer<typeof CaseSchema>;

const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/host/encode.json")).map(
  (raw) => CaseSchema.parse(raw),
);

function encoded(c: Case): z.infer<typeof ExpectSchema> {
  const decision: Decision =
    c.gateClass === undefined ? c.decision : degradeAsk(c.host, c.event, c.decision, c.gateClass);
  try {
    const out = encodeDecision(c.host, c.event, decision);
    if (out.kind === "callback") return { callback: JSON.stringify(out) };
    expect(out.stderr).toBe("");
    expect(out.exitCode).toBe(0);
    return { stdout: out.stdout };
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
}

test("the fixture covers every host, event and decision", () => {
  const kinds = cases.map((c) => Object.keys(c.expect)[0]);
  expect(cases.length).toBe(299);
  expect(kinds.filter((kind) => kind === "stdout").length).toBe(238);
  expect(kinds.filter((kind) => kind === "callback").length).toBe(57);
  expect(kinds.filter((kind) => kind === "error").length).toBe(4);
});

for (const c of cases) {
  test(c.name, () => {
    expect(encoded(c)).toEqual(c.expect);
  });
}
