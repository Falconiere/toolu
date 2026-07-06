// Unit tests for the pure duration helper in activity/tree-builder.ts, using
// real ISO timestamps captured from Claude Code transcripts. No mocks.

import { describe, expect, test } from "bun:test";

import { durationMs } from "../activity/tree-builder.ts";

// Real spawn/result timestamps from a captured session (fixtures/transcript-main.jsonl).
const START = "2026-06-19T19:36:17.870Z";
const END = "2026-06-19T19:40:07.513Z";

describe("durationMs", () => {
  test("ordered timestamps → positive difference in ms", () => {
    expect(durationMs(START, END)).toBe(Date.parse(END) - Date.parse(START));
  });

  test("identical timestamps → 0", () => {
    expect(durationMs(START, START)).toBe(0);
  });

  test("out-of-order timestamps (end before start) → null, never negative", () => {
    expect(durationMs(END, START)).toBeNull();
  });

  test("missing either side → null", () => {
    expect(durationMs(null, END)).toBeNull();
    expect(durationMs(START, null)).toBeNull();
  });

  test("unparseable timestamp → null", () => {
    expect(durationMs("not-a-date", END)).toBeNull();
    expect(durationMs(START, "not-a-date")).toBeNull();
  });
});
