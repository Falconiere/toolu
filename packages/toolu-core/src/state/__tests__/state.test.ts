/**
 * The `@toolu/core/state` package export (#255) resolves through package.json
 * and carries the public surface later ports (#256, #258) build on.
 */
import { expect, test } from "bun:test";
import * as state from "@toolu/core/state";

test("@toolu/core/state exposes the state layer", () => {
  for (const name of [
    "recordGateFailure",
    "clearGateFile",
    "readGateFile",
    "sweepState",
    "diffSha",
    "telemetryAppend",
    "normalizeEditRecords",
    "formatEditRecords",
    "isEditTool",
    "branchSlug",
  ] as const) {
    expect(typeof state[name]).toBe("function");
  }
  expect(state.GATE_FILE_VERSION).toBe(1);
  expect(
    state.GateFileSchema.safeParse({ status: "passing", source: "s", updatedAt: "t" }).success,
  ).toBe(true);
});
