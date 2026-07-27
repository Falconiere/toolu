// Tests for the deduplicating reporter and its use in readJsonl.
//
// The dashboard's discovery/sync/fingerprint paths re-run on every poll tick
// (cfg.pollMs, default 1500ms). Reporting a persistent fault from one of those
// catch blocks on every tick is log spam; swallowing it is the bug the quality
// gate exists to catch. warnOnce is the middle: report each distinct fault once.

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { resetWarnOnce, warnOnce } from "../log.ts";
import { readJsonl } from "../transcript.ts";

let lines: string[];
let restore: () => void;

beforeEach(() => {
  resetWarnOnce();
  lines = [];
  const original = console.error;
  console.error = (...args: unknown[]) => {
    lines.push(args.map((a) => String(a)).join(" "));
  };
  restore = () => {
    console.error = original;
  };
});

afterEach(() => {
  restore();
  resetWarnOnce();
});

describe("warnOnce", () => {
  test("reports the first occurrence of a key and stays quiet after", () => {
    for (let i = 0; i < 50; i++) warnOnce("k", "boom");
    expect(lines).toEqual(["boom"]);
  });

  test("distinct keys each report once", () => {
    warnOnce("a", "first");
    warnOnce("b", "second");
    warnOnce("a", "first again");
    warnOnce("b", "second again");
    expect(lines).toEqual(["first", "second"]);
  });

  test("the message of the first call is the one reported", () => {
    warnOnce("k", "original");
    warnOnce("k", "later");
    expect(lines).toEqual(["original"]);
  });
});

describe("readJsonl", () => {
  let dir: string;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "toolu-jsonl-"));
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  const write = (name: string, body: string): string => {
    const p = join(dir, name);
    writeFileSync(p, body, "utf8");
    return p;
  };

  test("parses whole objects and skips blank lines silently", () => {
    const p = write("clean.jsonl", '{"a":1}\n\n{"b":2}\n');
    expect(readJsonl(p)).toEqual([{ a: 1 }, { b: 2 }]);
    expect(lines).toEqual([]);
  });

  test("a single torn tail line is the normal live-read case and is not reported", () => {
    const p = write("torn.jsonl", '{"a":1}\n{"b":2}\n{"c":');
    expect(readJsonl(p)).toEqual([{ a: 1 }, { b: 2 }]);
    expect(lines).toEqual([]);
  });

  test("more than one unparseable line is real corruption and is reported", () => {
    const p = write("corrupt.jsonl", '{"a":1}\nnot json\nalso not json\n');
    expect(readJsonl(p)).toEqual([{ a: 1 }]);
    expect(lines.length).toBe(1);
    expect(lines[0]).toContain("2 unparseable lines");
    expect(lines[0]).toContain(p);
  });

  test("a corrupt file re-read every tick is reported once, not once per read", () => {
    const p = write("corrupt.jsonl", "nope\nstill nope\n");
    for (let i = 0; i < 10; i++) readJsonl(p);
    expect(lines.length).toBe(1);
  });

  test("two different corrupt files are both reported", () => {
    const a = write("a.jsonl", "nope\nstill nope\n");
    const b = write("b.jsonl", "nope\nstill nope\n");
    readJsonl(a);
    readJsonl(b);
    expect(lines.length).toBe(2);
  });

  test("an unreadable path yields an empty array without throwing", () => {
    expect(readJsonl(join(dir, "missing.jsonl"))).toEqual([]);
  });

  test("top-level non-objects are dropped, not surfaced as records", () => {
    const p = write("scalars.jsonl", '{"a":1}\n[1,2]\nnull\n"str"\n');
    expect(readJsonl(p)).toEqual([{ a: 1 }]);
    // Arrays/null/strings parse fine — they are filtered by shape, not by the
    // catch — so nothing is counted as unparseable and nothing is reported.
    expect(lines).toEqual([]);
  });
});
