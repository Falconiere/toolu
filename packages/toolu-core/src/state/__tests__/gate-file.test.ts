/**
 * Gate-file behaviour that goes past bash (#255): strict reads classify an
 * unknown field or a foreign version as unrecognized. Recording replaces such
 * a file, with a warning and a breadcrumb. Clearing leaves it byte-identical.
 * Every other failure path never throws.
 */
import { describe, expect, test } from "bun:test";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { clearGateFile, readGateFile, recordGateFailure } from "../gate-file.ts";

const OLD = "2020-01-01T00:00:00Z";
const NOW = new Date("2026-09-28T10:00:00.500Z");
const ENTRY = { source: "ts-quality-hook", reason: "r", violations: "v\n", updatedAt: OLD };
const FAILING = {
  status: "failing",
  reason: "r",
  source: "ts-quality-hook",
  file: "/r/a.ts",
  violations: "v\n",
  entries: { "/r/a.ts": ENTRY, "/r/b.ts": ENTRY },
  updatedAt: OLD,
};

function gate(sb: Sandbox, body?: string): string {
  const dir = join(sb.project, ".claude", "tmp");
  mkdirSync(dir, { recursive: true });
  const file = join(dir, "quality-gate-status.json");
  if (body !== undefined) writeFileSync(file, body);
  return file;
}

function options(sb: Sandbox, warnings: string[]) {
  return {
    env: { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: "claude" },
    host: "claude" as const,
    now: () => NOW,
    warn: (m: string) => warnings.push(m),
  };
}

describe("readGateFile", () => {
  test.each([
    ["missing", undefined, "missing"],
    ["malformed", "{", "malformed"],
    ["null", "null", "malformed"],
    ["false", "false", "malformed"],
    ["empty", "", "malformed"],
    ["array", "[]", "unrecognized"],
    ["unknown field", JSON.stringify({ ...FAILING, owner: "x" }), "unrecognized"],
    ["version 2", JSON.stringify({ ...FAILING, version: 2 }), "unrecognized"],
    ["valid", JSON.stringify(FAILING), "ok"],
    ["valid with version 1", JSON.stringify({ ...FAILING, version: 1 }), "ok"],
  ])("%s → %s", (_name, body, kind) => {
    using sb = createSandbox();
    expect(String(readGateFile(gate(sb, body)).kind)).toBe(kind);
  });

  test("an unrecognized document names the offending field", () => {
    using sb = createSandbox();
    const read = readGateFile(
      gate(sb, JSON.stringify({ ...FAILING, entries: { a: { ...ENTRY, x: 1 } } })),
    );
    expect(read.kind === "unrecognized" && read.reason).toContain("entries.a");
  });
});

describe("unrecognized documents", () => {
  const cases = [
    ["an unknown top-level field", { ...FAILING, owner: "someone" }],
    ["version 2", { ...FAILING, version: 2 }],
  ] as const;

  for (const [name, doc] of cases) {
    test(`record replaces ${name}, warns, and logs the drop`, () => {
      using sb = createSandbox({ git: true, branch: "feat/x" });
      const file = gate(sb, JSON.stringify(doc, null, 2));
      const warnings: string[] = [];
      recordGateFailure(file, "/r/c.ts", "ts-quality-hook", "reason", "c\n", options(sb, warnings));
      const read = readGateFile(file);
      expect(read.kind).toBe("ok");
      expect(read.kind === "ok" && read.doc).toEqual({
        status: "failing",
        reason: "reason",
        source: "ts-quality-hook",
        file: "/r/c.ts",
        violations: "c\n",
        entries: {
          "/r/c.ts": {
            source: "ts-quality-hook",
            reason: "reason",
            violations: "c\n",
            updatedAt: "2026-09-28T10:00:00Z",
          },
        },
        updatedAt: "2026-09-28T10:00:00Z",
      });
      expect(warnings).toHaveLength(1);
      expect(warnings[0]).toStartWith(`gate-file: unrecognized gate file at ${file} (`);
      expect(readFileSync(`${file}.dropped.log`, "utf8")).toBe(
        "2026-09-28T10:00:00Z unrecognized gate file replaced; dropped 2 entry(ies)\n",
      );
    });

    test(`clear leaves ${name} byte-identical and emits no telemetry`, () => {
      using sb = createSandbox({ git: true, branch: "feat/x" });
      const body = JSON.stringify(doc, null, 2);
      const file = gate(sb, body);
      const warnings: string[] = [];
      expect(clearGateFile(file, "/r/a.ts", "ts-quality-hook", options(sb, warnings))).toBe("noop");
      expect(readFileSync(file, "utf8")).toBe(body);
      expect(warnings).toHaveLength(1);
      expect(warnings[0]).toEndWith("; ignoring clear");
      expect(existsSync(join(sb.project, ".claude", "tmp", "telemetry"))).toBe(false);
    });
  }
});

describe("failure paths never throw", () => {
  test("clear on a malformed file warns like bash and keeps it", () => {
    using sb = createSandbox();
    const file = gate(sb, "{oops");
    const warnings: string[] = [];
    expect(clearGateFile(file, "/r/a.ts", "s", options(sb, warnings))).toBe("noop");
    expect(warnings).toEqual([
      `gate-file: malformed JSON at ${file}; ignoring clear (gate stays failing until next write)`,
    ]);
    expect(readFileSync(file, "utf8")).toBe("{oops");
  });

  test("clear with no file is a silent no-op", () => {
    using sb = createSandbox();
    const warnings: string[] = [];
    expect(clearGateFile(gate(sb), "/r/a.ts", "s", options(sb, warnings))).toBe("noop");
    expect(warnings).toEqual([]);
  });

  test("record into a read-only directory falls back, reports dropped entries, and does not throw", () => {
    using sb = createSandbox();
    const file = gate(sb, JSON.stringify(FAILING, null, 2));
    const dir = join(sb.project, ".claude", "tmp");
    const warnings: string[] = [];
    chmodSync(dir, 0o555);
    try {
      recordGateFailure(file, "/r/c.ts", "s", "r", "v", options(sb, warnings));
    } finally {
      chmodSync(dir, 0o755);
    }
    expect(warnings).toContain(
      `gate-file: primary write failed at ${file}; single-slot fallback dropped 2 other entry(ies)`,
    );
  });

  test("record creates the telemetry line under the gate file's root", () => {
    using sb = createSandbox({ git: true, branch: "feat/x" });
    const file = gate(sb);
    recordGateFailure(file, "/r/a.ts", "ts-quality-hook", "r", "v", options(sb, []));
    expect(
      readFileSync(join(sb.project, ".claude", "tmp", "telemetry", "feat_x.jsonl"), "utf8"),
    ).toBe(
      '{"file":"/r/a.ts","source":"ts-quality-hook","v":1,"t":"2026-09-28T10:00:00Z","branch":"feat/x","event":"gate_fail"}\n',
    );
  });
});
