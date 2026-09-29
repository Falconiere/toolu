/**
 * Concurrent writers never corrupt a gate file (#255, AC-2). The writers are
 * real OS processes racing on one real file, and a reader polls it for the
 * whole race. TypeScript writers serialize on `<gate>.lock`, so no entry is
 * lost either. Bash writers take no lock and can drop a merge, but the file
 * is still always one whole, valid document.
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readdirSync, utimesSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { readGateFile } from "../gate-file.ts";

const WRITER = resolve(import.meta.dir, "gate-writer.ts");
const GATE_FILE_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/gate-file.sh",
);

function setup(): { sb: Sandbox; gate: string; env: Record<string, string> } {
  const sb = createSandbox({ git: true, branch: "feat/race" });
  sb.writeConfig("claude", "project", { version: 1 });
  const dir = join(sb.project, ".claude", "tmp");
  mkdirSync(dir, { recursive: true });
  const env = { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: "claude" };
  return { sb, gate: join(dir, "quality-gate-status.json"), env };
}

/** Poll the gate file until `done` settles; record every read that is neither missing nor valid. */
async function watch(gate: string, done: Promise<unknown>): Promise<string[]> {
  const bad: string[] = [];
  let finished = false;
  void done.finally(() => {
    finished = true;
  });
  let reads = 0;
  while (!finished) {
    const read = readGateFile(gate);
    if (read.kind !== "ok" && read.kind !== "missing") bad.push(`${read.kind}: ${read.reason}`);
    reads += 1;
    await Bun.sleep(1);
  }
  expect(reads).toBeGreaterThan(5);
  return bad;
}

function expectAllOk(results: RunResult[]): void {
  for (const res of results) {
    expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
  }
}

test("16 concurrent TypeScript writers: valid file, every entry kept, no temp or lock left", async () => {
  const { sb, gate, env } = setup();
  using _sb = sb;
  const writers = Array.from({ length: 16 }, (_, id) =>
    run(["bun", WRITER, gate, String(id), "4", "strict"], {
      cwd: sb.project,
      env,
      timeoutMs: 60_000,
    }),
  );
  const all = Promise.all(writers);
  const bad = await watch(gate, all);
  expectAllOk(await all);
  expect(bad).toEqual([]);

  const read = readGateFile(gate);
  if (read.kind !== "ok" || read.doc.status !== "failing")
    throw new Error(`unexpected ${read.kind}`);
  const expected = Array.from({ length: 16 }, (_, id) => [
    `/w/${String(id)}/1`,
    `/w/${String(id)}/3`,
  ]).flat();
  expect(Object.keys(read.doc.entries ?? {}).sort()).toEqual(expected.sort());
  // violations aggregates exactly the surviving entries.
  expect(read.doc.violations.split("\n").filter(Boolean).sort()).toEqual(
    expected.map((file) => file.slice(3)).sort(),
  );
  expect(
    readdirSync(join(sb.project, ".claude", "tmp")).filter((n) => /\.(tmp|lock)$/.test(n)),
  ).toEqual([]);
}, 120_000);

test("8 bash and 8 TypeScript writers racing: the file is never torn and always valid", async () => {
  const { sb, gate, env } = setup();
  using _sb = sb;
  const ts = Array.from({ length: 8 }, (_, id) =>
    run(["bun", WRITER, gate, `ts${String(id)}`, "3", "lenient"], {
      cwd: sb.project,
      env,
      timeoutMs: 60_000,
    }),
  );
  const bash = Array.from({ length: 8 }, (_, id) =>
    run(
      [
        "bash",
        "-c",
        '. "$1"; for n in 0 1 2; do gate_record_failure "$2" "/w/sh$3/$n" "writer-sh$3" "reason" "sh$3/$n"; done',
        "_",
        GATE_FILE_SH,
        gate,
        String(id),
      ],
      { cwd: sb.project, env, timeoutMs: 60_000 },
    ),
  );
  const all = Promise.all([...ts, ...bash]);
  const bad = await watch(gate, all);
  expectAllOk(await all);
  expect(bad).toEqual([]);

  const read = readGateFile(gate);
  expect(read.kind).toBe("ok");
  // An unlocked bash merge may drop an entry, or restore one a TypeScript writer
  // just cleared from its stale read. Every survivor still came from these writers.
  const keys =
    read.kind === "ok" && read.doc.status === "failing" ? Object.keys(read.doc.entries ?? {}) : [];
  expect(keys.length).toBeGreaterThan(0);
  for (const key of keys) expect(key).toMatch(/^\/w\/(ts|sh)[0-7]\/[0-2]$/);
}, 120_000);

test("a stale lock from a crashed writer does not block the next one", async () => {
  const { sb, gate, env } = setup();
  using _sb = sb;
  writeFileSync(`${gate}.lock`, "4242 crashed\n");
  const old = new Date(Date.now() - 60_000);
  utimesSync(`${gate}.lock`, old, old);
  const started = Date.now();
  expectAllOk([await run(["bun", WRITER, gate, "solo", "2", "strict"], { cwd: sb.project, env })]);
  // Broken immediately, not after the 5 s wait.
  expect(Date.now() - started).toBeLessThan(5000);
  expect(existsSync(`${gate}.lock`)).toBe(false);
  const read = readGateFile(gate);
  expect(
    read.kind === "ok" && read.doc.status === "failing" && Object.keys(read.doc.entries ?? {}),
  ).toEqual(["/w/solo/1"]);
});
