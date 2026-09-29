/**
 * The jq-semantics helpers (#256) against the real jq binary: each case runs
 * the jq filter the bash libs use and the TypeScript helper over the same
 * input. Both must succeed with the same value, or both must fail.
 */
import { describe, expect, test } from "bun:test";
import { toJqJson } from "../../state/state-io.ts";
import {
  JqError,
  alt,
  concat,
  each,
  get,
  holds,
  length,
  raw,
  toStr,
  type Json,
} from "../ledger-jq.ts";

type Outcome = { ok: true; out: string } | { ok: false };

function jq(filter: string, input: Json, flags: string[] = ["-c"]): Outcome {
  const res = Bun.spawnSync(["jq", ...flags, filter], {
    stdin: new TextEncoder().encode(JSON.stringify(input)),
    stdout: "pipe",
    stderr: "pipe",
  });
  return res.success ? { ok: true, out: res.stdout.toString().replace(/\n$/, "") } : { ok: false };
}

function ts(fn: () => string): Outcome {
  try {
    return { ok: true, out: fn() };
  } catch (error) {
    if (error instanceof JqError) return { ok: false };
    throw error;
  }
}

const SHAPES: Json[] = [
  null,
  false,
  true,
  0,
  7,
  -2.5,
  "",
  "AC-1 AC-2",
  [],
  ["AC-1", 3, null],
  {},
  { a: 1, status: "green", id: "s1" },
  { "AC-1": [1] },
  { "AC-1": [null] },
  { "AC-1": "x" },
];

describe("get: .status", () => {
  for (const value of SHAPES) {
    test(JSON.stringify(value), () => {
      expect(ts(() => JSON.stringify(get(value, "status")))).toEqual(jq(".status", value));
    });
  }
});

describe("each: [.[]]", () => {
  for (const value of SHAPES) {
    test(JSON.stringify(value), () => {
      expect(ts(() => JSON.stringify(each(value)))).toEqual(jq("[.[]]", value));
    });
  }
});

describe('alt: . // "d"', () => {
  for (const value of SHAPES) {
    test(JSON.stringify(value), () => {
      expect(ts(() => JSON.stringify(alt(value, "d")))).toEqual(jq('. // "d"', value));
    });
  }
});

describe("length", () => {
  for (const value of SHAPES) {
    test(JSON.stringify(value), () => {
      expect(ts(() => JSON.stringify(length(value)))).toEqual(jq("length", value));
    });
  }
});

describe('holds: index("AC-1") truthiness', () => {
  for (const value of SHAPES) {
    test(JSON.stringify(value), () => {
      expect(ts(() => JSON.stringify(holds(value, "AC-1")))).toEqual(
        jq('if index("AC-1") then true else false end', value),
      );
    });
  }
});

describe("toStr and raw", () => {
  for (const value of [...SHAPES, "a\u007fb", { nested: ["x", { y: null }] }]) {
    test(JSON.stringify(value), () => {
      expect(ts(() => toJqJson(toStr(value), false))).toEqual(jq("tostring", value));
      expect(ts(() => raw(value))).toEqual(jq(".", value, ["-r"]));
    });
  }
});

describe('concat: "p" + .', () => {
  for (const value of SHAPES) {
    test(JSON.stringify(value), () => {
      expect(ts(() => JSON.stringify(concat("p", value)))).toEqual(jq('"p" + .', value));
    });
  }
});
