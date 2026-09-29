/**
 * `sweepState` vs bash `toolu_sweep_state` (#255, AC-4). Twin real repos get
 * the same branches, backdated state files, gate file and telemetry. After
 * one sweep each, the surviving files and their bytes must match. Age rules
 * are pinned one minute either side of the TTL and retention boundaries, and
 * again under a custom config and with the sweep disabled.
 */
import { describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { isoSeconds } from "../state-io.ts";
import { sweepState } from "../state-sweeper.ts";

const SWEEPER_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/state-sweeper.sh",
);
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

type Fixture = {
  config?: object;
  gate?: (sb: Sandbox) => string;
  ttlHours: number;
  retentionDays: number;
};

/** One clock per test, so both twins get identical telemetry stamps. */
let base = Date.now();

/** Branches: main (base), feat/merged (merged), feat/live-* (unmerged), feat/current (checked out); feat/gone is deleted. */
function arrangeBranches(sb: Sandbox): void {
  sb.git("branch", "feat/merged");
  for (const name of ["feat/live-fresh", "feat/live-stale", "feat/live-custom"]) {
    sb.git("checkout", "-q", "-b", name, "main");
    sb.write(`${name}.txt`, name);
    sb.git("add", "-A");
    sb.git("commit", "-q", "-m", name);
  }
  sb.git("checkout", "-q", "-b", "feat/gone", "main");
  sb.git("checkout", "-q", "-b", "feat/current", "main");
  sb.git("branch", "-q", "-D", "feat/gone");
}

function put(file: string, body: string, ageMs = 0): void {
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, body);
  const when = new Date(base - ageMs);
  utimesSync(file, when, when);
}

function arrangeState(sb: Sandbox, f: Fixture): void {
  const tmp = join(sb.project, ".claude", "tmp");
  const ttl = f.ttlHours * HOUR;
  put(join(tmp, "push-review", "feat_merged.json"), "{}");
  put(join(tmp, "push-review", "feat_gone.pending-waiver.json"), "{}");
  put(join(tmp, "push-review", "feat_live-fresh.json"), "{}", ttl - MIN);
  put(join(tmp, "push-review", "feat_live-stale.waiver.json"), "{}", ttl + MIN);
  put(join(tmp, "plan-ledger", "feat_live-custom.json"), "{}", 24 * HOUR + MIN);
  put(join(tmp, "plan-ledger", "feat_current.json"), "{}", 30 * DAY);
  put(join(tmp, "docs-sync", ".hidden.json"), "{}", 30 * DAY);
  put(join(tmp, "docs-sync", "notes.txt"), "keep", 30 * DAY);
  put(join(tmp, ".permissions-written"), "", 30 * DAY);

  const cutoff = base - f.retentionDays * DAY;
  const at = (ms: number) => isoSeconds(new Date(ms));
  const line = (t: unknown, extra: object = {}) =>
    JSON.stringify({ ...extra, v: 1, t, branch: "feat/current", event: "docs_nudge" });
  const telemetry = join(tmp, "telemetry");
  put(
    join(telemetry, "feat_current.jsonl"),
    [
      line(at(cutoff - MIN)),
      line(at(cutoff + MIN)).replace("{", '{"d":"caf\\u00e9",'),
      "null",
      "",
      `{"t": ${JSON.stringify(at(cutoff + 2 * MIN))} , "x":[1, 2]}`,
      line(5),
      line({ o: 1 }),
    ].join("\n") + "\n",
  );
  put(join(telemetry, "old.jsonl"), `${line(at(cutoff - DAY))}\n`);
  put(join(telemetry, "broken.jsonl"), `${line(at(cutoff - DAY))}\n{broken\n`);
  put(join(telemetry, "scalar.jsonl"), `${line(at(cutoff - DAY))}\n5\n`);
  put(join(telemetry, "ignored.txt"), `${line(at(cutoff - DAY))}\n`);
  if (f.gate !== undefined) put(join(tmp, "quality-gate-status.json"), f.gate(sb));
}

function arrange(f: Fixture): Sandbox {
  const sb = createSandbox({ git: true });
  arrangeBranches(sb);
  if (f.config !== undefined) sb.writeConfig("claude", "project", f.config);
  arrangeState(sb, f);
  return sb;
}

function env(sb: Sandbox): Record<string, string> {
  return { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: "claude" };
}

/** Relative path → content for everything left under `.claude/tmp`. */
function survivors(sb: Sandbox): Record<string, string> {
  const root = join(sb.project, ".claude", "tmp");
  const out: Record<string, string> = {};
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else
        out[relative(root, path)] = readFileSync(path, "utf8").replaceAll(sb.project, "<project>");
    }
  };
  if (existsSync(root)) walk(root);
  return out;
}

async function viaBash(f: Fixture): Promise<Record<string, string>> {
  using sb = arrange(f);
  const res = await run(
    ["bash", "-c", '. "$1"; toolu_sweep_state "$2"; echo "rc=$?"', "_", SWEEPER_SH, sb.project],
    {
      cwd: sb.project,
      env: env(sb),
    },
  );
  expect(res.stdout.trim()).toBe("rc=0");
  return survivors(sb);
}

function viaTs(f: Fixture): Record<string, string> {
  using sb = arrange(f);
  const warnings: string[] = [];
  sweepState(sb.project, { env: env(sb), host: "claude", warn: (m) => warnings.push(m) });
  expect(warnings).toEqual([]);
  return survivors(sb);
}

const gate = (doc: object | ((sb: Sandbox) => object)) => (sb: Sandbox) =>
  `${JSON.stringify(typeof doc === "function" ? doc(sb) : doc, null, 2)}\n`;
const OLD = "2020-01-01T00:00:00Z";
const failing = (keys: string[]) => ({
  status: "failing",
  reason: "r",
  source: "ts-quality-hook",
  file: keys.at(-1) ?? "",
  violations: "v",
  entries: Object.fromEntries(
    keys.map((key) => [
      key,
      { source: "ts-quality-hook", reason: "r", violations: "v", updatedAt: OLD },
    ]),
  ),
  updatedAt: OLD,
});

const DEFAULTS = { ttlHours: 24, retentionDays: 7 };
const FIXTURES: Record<string, Fixture> = {
  "defaults, passing gate": {
    ...DEFAULTS,
    gate: gate({ status: "passing", source: "bun test", updatedAt: OLD }),
  },
  "defaults, 30-day-old failure about a live file": {
    ...DEFAULTS,
    gate: gate((sb) => failing([join(sb.project, ".gitkeep"), "/gone/b.ts"])),
  },
  "defaults, failure only about gone files": {
    ...DEFAULTS,
    gate: gate(failing(["/gone/a.ts", "/gone/b.ts"])),
  },
  "defaults, legacy __global__ failure": {
    ...DEFAULTS,
    gate: gate({
      status: "failing",
      reason: "r",
      source: "gate-status-hook",
      file: "__global__",
      violations: "",
      updatedAt: OLD,
    }),
  },
  "defaults, legacy failure about a gone file": {
    ...DEFAULTS,
    gate: gate({
      status: "failing",
      reason: "r",
      source: "s",
      file: "/gone/x.ts",
      violations: "",
      updatedAt: OLD,
    }),
  },
  "defaults, empty-string key is not a live violation": { ...DEFAULTS, gate: gate(failing([""])) },
  "defaults, malformed gate kept": { ...DEFAULTS, gate: () => "{nope" },
  "custom TTL 48h and retention 1 day": {
    config: { version: 1, gates: { stateTtlHours: 48.9, telemetryRetentionDays: 1 } },
    ttlHours: 48,
    retentionDays: 1,
    gate: gate({ status: "passing", source: "s", updatedAt: OLD }),
  },
  "non-positive and non-numeric settings fall back to defaults": {
    config: { version: 1, gates: { stateTtlHours: 0, telemetryRetentionDays: "3" } },
    ...DEFAULTS,
  },
  "sweep disabled": {
    config: { version: 1, gates: { sweep: false } },
    ...DEFAULTS,
    gate: gate({ status: "passing", source: "s", updatedAt: OLD }),
  },
};

describe("bash and TypeScript leave the same state behind", () => {
  for (const [name, fixture] of Object.entries(FIXTURES)) {
    test(
      name,
      async () => {
        base = Date.now();
        const bash = await viaBash(fixture);
        expect(viaTs(fixture)).toEqual(bash);
      },
      60_000,
    );
  }
});

test("the age rules hold at their boundaries (defaults)", async () => {
  base = Date.now();
  const kept = Object.keys(await viaBash(FIXTURES["defaults, passing gate"] ?? DEFAULTS)).sort();
  expect(kept).toEqual([
    ".permissions-written",
    "docs-sync/.hidden.json",
    "docs-sync/notes.txt",
    "plan-ledger/feat_current.json",
    "push-review/feat_live-fresh.json",
    "telemetry/broken.jsonl",
    "telemetry/feat_current.jsonl",
    "telemetry/ignored.txt",
    "telemetry/scalar.jsonl",
  ]);
  const ts = viaTs(FIXTURES["defaults, passing gate"] ?? DEFAULTS);
  expect(ts["telemetry/feat_current.jsonl"]?.split("\n").filter(Boolean)).toHaveLength(3);
  expect(ts["telemetry/feat_current.jsonl"]).toContain('"d":"café"');
}, 60_000);

test("an unrecognized gate file is kept, where bash would judge it (documented divergence)", () => {
  const doc = { ...failing(["/gone/a.ts"]), owner: "future-writer" };
  const fixture = { ...DEFAULTS, gate: gate(doc) };
  const ts = viaTs(fixture);
  expect(ts["quality-gate-status.json"]).toBe(`${JSON.stringify(doc, null, 2)}\n`);
});

test("a failure about a live file survives at any age; one about gone files is reclaimed", () => {
  base = Date.now();
  const live = viaTs(FIXTURES["defaults, 30-day-old failure about a live file"] ?? DEFAULTS);
  expect(live["quality-gate-status.json"]).toContain("<project>/.gitkeep");
  expect(
    viaTs(FIXTURES["defaults, legacy __global__ failure"] ?? DEFAULTS)["quality-gate-status.json"],
  ).toBeDefined();
  expect(
    viaTs(FIXTURES["defaults, failure only about gone files"] ?? DEFAULTS)[
      "quality-gate-status.json"
    ],
  ).toBeUndefined();
  expect(viaTs(FIXTURES["sweep disabled"] ?? DEFAULTS)["push-review/feat_merged.json"]).toBe("{}");
});
