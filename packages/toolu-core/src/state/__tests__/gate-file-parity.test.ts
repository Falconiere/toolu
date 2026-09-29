/**
 * `recordGateFailure`/`clearGateFile` vs bash `gate_record_failure`/
 * `gate_clear_file` (#255). Twin real repos start from the same gate file and
 * run the same operations. The gate file, its `.dropped.log` and the
 * gate_fail/gate_clear telemetry must match byte for byte once this run's
 * timestamps are normalized. Seed timestamps are in 2020, so they stay
 * literal. File keys ascend in call order, so the (updatedAt, key) sort agrees
 * whichever second each call lands in. A second pass alternates bash and
 * TypeScript on one file.
 */
import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { clearGateFile, readGateFile, recordGateFailure } from "../gate-file.ts";

const GATE_FILE_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/gate-file.sh",
);
const RUN_STAMP = /20(?!20-)\d\d-\d\d-\d\dT\d\d:\d\d:\d\dZ/g;

type Op =
  | { op: "record"; file: string; source: string; reason: string; violations: string }
  | { op: "clear"; file: string; source: string };

type Scenario = { seed?: string; ops: Op[] };

const OLD = "2020-01-01T00:00:00Z";
const ENTRY = (source: string, reason: string, violations: string, updatedAt = OLD) => ({
  source,
  reason,
  violations,
  updatedAt,
});
const LEGACY = {
  status: "failing",
  reason: "Quality command failed: bun test (exit 1)",
  source: "gate-status-hook",
  file: "__global__",
  violations: "",
  updatedAt: OLD,
};
const MULTI = {
  status: "failing",
  reason: "r2",
  source: "rust-quality-hook",
  file: "/r/z.rs",
  violations: "v1\nv2\n",
  entries: {
    "/r/y.ts": ENTRY("ts-quality-hook", "r1", "v1\n"),
    "/r/z.rs": ENTRY("rust-quality-hook", "r2", "v2\n", "2020-01-02T00:00:00Z"),
  },
  updatedAt: "2020-01-02T00:00:00Z",
};

const rec = (
  file: string,
  source = "ts-quality-hook",
  reason = "Post-edit quality violation(s) detected",
  violations = `${file}: bad\n`,
): Op => ({
  op: "record",
  file,
  source,
  reason,
  violations,
});
const clr = (file: string, source = "ts-quality-hook"): Op => ({ op: "clear", file, source });
const pretty = (doc: object) => `${JSON.stringify(doc, null, 2)}\n`;

const SCENARIOS: Record<string, Scenario> = {
  "no file: record, re-record, wrong-source clear, clear to passing": {
    ops: [
      rec("/r/a.ts"),
      rec("/r/b.ts"),
      rec("/r/a.ts", "ts-quality-hook", "again", "new\n"),
      clr("/r/b.ts", "rust-quality-hook"),
      clr("/r/a.ts"),
      clr("/r/b.ts"),
    ],
  },
  "legacy single-slot seed is promoted into entries": {
    seed: pretty(LEGACY),
    ops: [rec("/r/a.ts"), clr("__global__", "gate-status-hook"), clr("/r/a.ts")],
  },
  "legacy seed cleared by its own source and file": {
    seed: pretty(LEGACY),
    ops: [clr("__global__", "gate-status-hook")],
  },
  "multi-slot seed: add, replace in place, clear the older": {
    seed: pretty(MULTI),
    ops: [
      rec("/r/x.py", "python-quality-hook"),
      rec("/r/y.ts", "ts-quality-hook", "r1b", "v1b\n"),
      clr("/r/z.rs", "rust-quality-hook"),
    ],
  },
  "malformed seed: clear ignores it, record starts fresh": {
    seed: "{not json",
    ops: [clr("/r/a.ts"), rec("/r/a.ts")],
  },
  "null seed is malformed": { seed: "null\n", ops: [rec("/r/a.ts")] },
  "empty seed is malformed": { seed: "", ops: [rec("/r/a.ts")] },
  "passing seed: record then clear": {
    seed: pretty({ status: "passing", source: "bun test", updatedAt: OLD }),
    ops: [clr("/r/a.ts"), rec("/r/a.ts"), clr("/r/a.ts")],
  },
  "DEL, unicode and newlines in reason and violations": {
    ops: [
      rec("/r/a\u007f.ts", "ts-quality-hook", "reason é \u007f ～ 😀", "line1\n\tline2 \u2028\n"),
      rec("/r/b.ts"),
      clr("/r/a\u007f.ts"),
    ],
  },
  "same-second failures tie-break by key": {
    ops: [rec("/r/1.ts"), rec("/r/2.ts"), rec("/r/3.ts"), rec("/r/4.ts"), clr("/r/4.ts")],
  },
  'empty-string source clears a missing entry (jq `// ""` parity)': {
    seed: pretty(MULTI),
    ops: [clr("/r/none.ts", "")],
  },
};

type Impl = "bash" | "ts";

function repo(): Sandbox {
  const sb = createSandbox({ git: true, branch: "feat/gate" });
  sb.writeConfig("claude", "project", { version: 1 });
  return sb;
}

function env(sb: Sandbox): Record<string, string> {
  return { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: "claude" };
}

function gatePath(sb: Sandbox): string {
  return join(sb.project, ".claude", "tmp", "quality-gate-status.json");
}

async function apply(sb: Sandbox, impl: Impl, op: Op): Promise<void> {
  const gate = gatePath(sb);
  if (impl === "ts") {
    const options = { env: env(sb), host: "claude" as const, warn: () => {} };
    if (op.op === "record")
      recordGateFailure(gate, op.file, op.source, op.reason, op.violations, options);
    else clearGateFile(gate, op.file, op.source, options);
    return;
  }
  const args =
    op.op === "record"
      ? [
          'gate_record_failure "$2" "$3" "$4" "$5" "$6"',
          gate,
          op.file,
          op.source,
          op.reason,
          op.violations,
        ]
      : ['gate_clear_file "$2" "$3" "$4"', gate, op.file, op.source];
  const [call, ...rest] = args;
  const res = await run(["bash", "-c", `. "$1"; ${call ?? ""}`, "_", GATE_FILE_SH, ...rest], {
    cwd: sb.project,
    env: env(sb),
  });
  expect(res.exitCode).toBe(0);
}

/** Everything the gate writers leave under the state root, run stamps normalized. */
function snapshot(sb: Sandbox): Record<string, string> {
  const out: Record<string, string> = {};
  const tmp = join(sb.project, ".claude", "tmp");
  for (const rel of ["quality-gate-status.json", "quality-gate-status.json.dropped.log"]) {
    const path = join(tmp, rel);
    if (existsSync(path)) out[rel] = readFileSync(path, "utf8").replace(RUN_STAMP, "NOW");
  }
  const telemetry = join(tmp, "telemetry");
  for (const name of existsSync(telemetry) ? readdirSync(telemetry) : []) {
    out[`telemetry/${name}`] = readFileSync(join(telemetry, name), "utf8").replace(
      RUN_STAMP,
      "NOW",
    );
  }
  // No temp or lock file survives either implementation.
  expect(
    readdirSync(tmp).filter((name) => /\.(tmp|lock)$|\.json\.[A-Za-z0-9]{6}$/.test(name)),
  ).toEqual([]);
  return out;
}

async function play(
  scenario: Scenario,
  pick: (index: number) => Impl,
): Promise<Record<string, string>> {
  using sb = repo();
  mkdirSync(join(sb.project, ".claude", "tmp"), { recursive: true });
  if (scenario.seed !== undefined) writeFileSync(gatePath(sb), scenario.seed);
  for (const [index, op] of scenario.ops.entries()) {
    await apply(sb, pick(index), op);
  }
  const bashWritten = readGateFile(gatePath(sb));
  const snap = snapshot(sb);
  // AC-9: whatever the writers leave behind is a valid v1 document.
  if (bashWritten.kind !== "missing") expect(bashWritten.kind).toBe("ok");
  return snap;
}

describe("bash and TypeScript write the same bytes", () => {
  for (const [name, scenario] of Object.entries(SCENARIOS)) {
    test(name, async () => {
      const bash = await play(scenario, () => "bash");
      const ts = await play(scenario, () => "ts");
      expect(ts).toEqual(bash);
      expect(bash["quality-gate-status.json"]).toBeDefined();
    });
  }
});

describe("bash and TypeScript alternate on one file", () => {
  for (const [name, scenario] of Object.entries(SCENARIOS)) {
    test(name, async () => {
      const bash = await play(scenario, () => "bash");
      const mixed = await play(scenario, (index) => (index % 2 === 0 ? "ts" : "bash"));
      const flipped = await play(scenario, (index) => (index % 2 === 0 ? "bash" : "ts"));
      expect(mixed).toEqual(bash);
      expect(flipped).toEqual(bash);
    });
  }
});
