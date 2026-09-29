/**
 * The golden file and the case matrix stay in lockstep (#263): every case has
 * a bash capture and no capture outlives its case.
 */
import { expect, test } from "bun:test";
import { readGolden } from "./lifecycle-golden.ts";
import { SESSION_START_CASES } from "./session-start-cases.ts";
import { USER_PROMPT_SUBMIT_CASES } from "./user-prompt-submit-cases.ts";

test("every case has exactly one bash capture", () => {
  const names = [...SESSION_START_CASES, ...USER_PROMPT_SUBMIT_CASES].map((c) => c.name);
  expect(new Set(names).size).toBe(names.length);
  expect(Object.keys(readGolden().cases).toSorted()).toEqual(names.toSorted());
});

test("the captures name the bash base they came from", () => {
  expect(readGolden().base).toBe("107e566f");
});
