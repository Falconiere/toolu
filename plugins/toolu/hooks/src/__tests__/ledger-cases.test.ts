/**
 * The shared ledger golden (#421): `plan-ledger` and `verdict` reproduce every
 * run of `fixtures/ledger/cases.json`, its streams, exit code and branch
 * ledger. They are reached through the `TOOLU_IMPL` seam, so the same cases run
 * against the Bun bundles by default and `toolu ledger` when
 * `TOOLU_IMPL=rust:toolu/plan-ledger,toolu/verdict` selects them.
 */
import { expect, test } from "bun:test";
import { implementationTag } from "@toolu/conformance/harness/entry-command";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { CaseSchema, FIXTURE, runCase } from "./ledger-cases.ts";

const cases = readCaseFile(FIXTURE).map((raw) => CaseSchema.parse(raw));
const tag = `${implementationTag("toolu", "plan-ledger")}${implementationTag("toolu", "verdict")}`;

test.concurrent.each(cases.map((c) => [`${c.name}${tag}`, c] as const))(
  "%s",
  async (_name, c) => {
    await runCase(c, (step, observed, index) => {
      const expected = { ...step.expect, ledger: step.expect.ledger ?? null };
      expect({ index, ...observed }).toEqual({ index, ...expected });
    });
  },
  60_000,
);
