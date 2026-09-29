/**
 * Strict v1 schemas (#255): every shape the bash writers produce is accepted;
 * an unknown field at any level, or a version other than 1, is rejected.
 */
import { describe, expect, test } from "bun:test";
import {
  EditRecordSchema,
  GateFileSchema,
  TELEMETRY_EXTRAS,
  TelemetryLineSchema,
} from "../state-schema.ts";

const ENTRY = {
  source: "ts-quality-hook",
  reason: "r",
  violations: "v\n",
  updatedAt: "2026-01-01T00:00:00Z",
};
const PASSING = { status: "passing", source: "bun test", updatedAt: "2026-01-01T00:00:00Z" };
const LEGACY = {
  status: "failing",
  reason: "Quality command failed: bun test (exit 1)",
  source: "gate-status-hook",
  file: "__global__",
  violations: "",
  updatedAt: "2026-01-01T00:00:00Z",
};
const MULTI = { ...LEGACY, entries: { "/repo/a.ts": ENTRY, __global__: ENTRY } };

describe("GateFileSchema", () => {
  test.each([
    ["passing", PASSING],
    ["legacy single-slot failing", LEGACY],
    ["multi-slot failing", MULTI],
    ["explicit version 1", { ...MULTI, version: 1 }],
  ])("accepts %s", (_name, doc) => {
    expect(GateFileSchema.safeParse(doc).success).toBe(true);
  });

  test.each([
    ["unknown top-level key", { ...MULTI, owner: "x" }],
    ["unknown entry key", { ...MULTI, entries: { a: { ...ENTRY, extra: 1 } } }],
    ["unknown key on passing", { ...PASSING, file: "a" }],
    ["version 2", { ...MULTI, version: 2 }],
    ["unknown status", { ...PASSING, status: "unknown" }],
    ["non-string violations", { ...LEGACY, violations: 3 }],
    ["entries not an object", { ...LEGACY, entries: [] }],
  ])("rejects %s", (_name, doc) => {
    expect(GateFileSchema.safeParse(doc).success).toBe(false);
  });
});

describe("TelemetryLineSchema", () => {
  const protocol = { v: 1, t: "2026-01-01T00:00:00Z", branch: "feat/x" };

  test("accepts each event with its exact extras", () => {
    const lines = [
      { file: "/a.ts", source: "ts-quality-hook", ...protocol, event: "gate_fail" },
      { file: "/a.ts", source: "ts-quality-hook", ...protocol, event: "gate_clear" },
      {
        step_id: "S1",
        status: "green",
        exit_code: 0,
        duration_s: 2,
        attempt: 1,
        ...protocol,
        event: "step_run",
      },
      { covered: 3, uncovered: 1, ...protocol, event: "ac_coverage" },
      { decision: "updated", ...protocol, event: "docs_attested" },
      { ...protocol, event: "docs_nudge" },
      { result: "deny", reason_code: "stale", round: null, ...protocol, event: "push_check" },
      {
        model: "sonnet",
        subagent_type: null,
        reasoning_effort: null,
        step_id: null,
        step_model: null,
        ...protocol,
        event: "delegation",
      },
    ];
    for (const line of lines) {
      expect(TelemetryLineSchema.safeParse(line).success).toBe(true);
    }
    expect(lines.map((line) => line.event).sort()).toEqual(Object.keys(TELEMETRY_EXTRAS).sort());
  });

  test.each([
    ["unknown event", { ...protocol, event: "shell" }],
    [
      "extra field",
      { decision: "x", command: "curl -H token", ...protocol, event: "docs_attested" },
    ],
    ["nested payload", { decision: { tool_input: {} }, ...protocol, event: "docs_attested" }],
    ["v 2", { ...protocol, v: 2, event: "docs_nudge" }],
    ["missing protocol field", { v: 1, t: protocol.t, event: "docs_nudge" }],
  ])("rejects %s", (_name, line) => {
    expect(TelemetryLineSchema.safeParse(line).success).toBe(false);
  });

  test("per-event extras reject protocol keys smuggled in by a caller", () => {
    expect(TELEMETRY_EXTRAS.docs_nudge.safeParse({ event: "gate_clear" }).success).toBe(false);
    expect(
      TELEMETRY_EXTRAS.gate_fail.safeParse({ file: "a", source: "b", branch: "main" }).success,
    ).toBe(false);
  });
});

describe("EditRecordSchema", () => {
  test.each([
    { path: "a.ts", operation: "update" },
    { path: "a.ts", operation: "update", moved_to: "b.ts" },
    { path: "b.ts", operation: "move", from: "a.ts" },
  ])("accepts %j", (record) => {
    expect(EditRecordSchema.safeParse(record).success).toBe(true);
  });

  test.each([
    { path: "", operation: "add" },
    { path: "a.ts", operation: "rename" },
    { path: "a.ts", operation: "add", content: "secret" },
  ])("rejects %j", (record) => {
    expect(EditRecordSchema.safeParse(record).success).toBe(false);
  });
});
