/** Comparing a TypeScript-module run with its bash capture (#268). */
import { expect } from "bun:test";
import type { Deviation } from "./cases-types.ts";

/** Each case spawns real git, Bun and bash processes, several times. */
export const CASE_TIMEOUT_MS = 60_000;

/** Where bash was wrong: `actual` shows the TypeScript answer and differs from the capture. */
export function expectDeviation(deviation: Deviation, actual: string, captured: string): void {
  expect(actual).not.toBe(captured);
  if ("silent" in deviation) {
    expect(actual).toBe("");
    return;
  }
  if (deviation.contains !== undefined) expect(actual).toContain(deviation.contains);
  if (deviation.excludes !== undefined) expect(actual).not.toContain(deviation.excludes);
}
