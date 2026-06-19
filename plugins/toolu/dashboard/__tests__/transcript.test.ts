// Real-data tests for the transcript reader + pairing. Fixture `transcript-main.jsonl`
// is captured from a real Claude Code session: two real Agent spawns (one with a
// real tool_result → done, one without → running) plus a deliberately truncated
// final line to prove tolerance (AC-15). No mocks.

import { describe, expect, test } from "bun:test";
import { join } from "node:path";

import { extractResults, extractSpawns, pairAgents, readJsonl } from "../transcript.ts";

const FIXTURE = join(import.meta.dir, "fixtures", "transcript-main.jsonl");
const SPAWN_DONE = "toolu_0172xDeCHo4xPYFm2M8MGb4G";
const SPAWN_RUNNING = "toolu_01CZjAm4dAWLvaZXKjiFvnCb";

describe("readJsonl (AC-15)", () => {
  test("parses all valid lines and skips the truncated final line — no throw", () => {
    const lines = readJsonl(FIXTURE);
    // 2 spawns + 1 result = 3 valid objects; the 4th truncated line is dropped
    expect(lines.length).toBe(3);
  });

  test("missing file returns [] (no throw)", () => {
    expect(readJsonl(join(import.meta.dir, "fixtures", "nope.jsonl"))).toEqual([]);
  });
});

describe("extractSpawns / extractResults", () => {
  const lines = readJsonl(FIXTURE);

  test("both real Agent spawns are extracted with descriptions", () => {
    const spawns = extractSpawns(lines);
    expect(spawns.has(SPAWN_DONE)).toBe(true);
    expect(spawns.has(SPAWN_RUNNING)).toBe(true);
    expect(spawns.get(SPAWN_DONE)!.name).toBe("Agent");
    expect(spawns.get(SPAWN_DONE)!.description.length).toBeGreaterThan(0);
    expect(spawns.get(SPAWN_DONE)!.startedAt).not.toBeNull();
  });

  test("only the resolved spawn has a result", () => {
    const results = extractResults(lines);
    expect(results.has(SPAWN_DONE)).toBe(true);
    expect(results.has(SPAWN_RUNNING)).toBe(false);
    expect(results.get(SPAWN_DONE)!.isError).toBe(false);
  });
});

describe("pairAgents", () => {
  const paired = pairAgents(readJsonl(FIXTURE));
  const byId = new Map(paired.map((p) => [p.toolUseId, p]));

  test("resolved spawn → ended + non-null duration", () => {
    const done = byId.get(SPAWN_DONE)!;
    expect(done.endedAt).not.toBeNull();
    expect(done.durationMs).not.toBeNull();
    expect(done.durationMs!).toBeGreaterThanOrEqual(0);
  });

  test("unresolved spawn → still running (endedAt null, duration null)", () => {
    const running = byId.get(SPAWN_RUNNING)!;
    expect(running.endedAt).toBeNull();
    expect(running.durationMs).toBeNull();
  });
});
