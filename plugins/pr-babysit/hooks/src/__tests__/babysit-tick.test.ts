import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../../../../..");
const snapshot = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/snapshots/toolu-165.json",
);
const nextEntry = join(root, "plugins/pr-babysit/hooks/src/babysit-tick.ts");
const dirs: string[] = [];

function fixture(): { dir: string; next: string } {
  const dir = mkdtempSync(join(tmpdir(), "babysit-tick-"));
  dirs.push(dir);
  return { dir, next: join(dir, "next.json") };
}
afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function invoke(
  state: string,
  extra: string[] = [],
): { status: number | null; output: Record<string, any> } {
  const run = spawnSync(
    process.execPath,
    [
      nextEntry,
      "--repo",
      "Falconiere/toolu",
      "--pr",
      "165",
      "--state-file",
      state,
      "--snapshot-in",
      snapshot,
      "--now",
      "2026-09-19T12:00:00Z",
      ...extra,
    ],
    { encoding: "utf8" },
  );
  return { status: run.status, output: JSON.parse(run.stdout) };
}

test("two ticks on a captured snapshot persist complete state and backoff", () => {
  const f = fixture();
  for (let round = 0; round < 2; round += 1) {
    const next = invoke(f.next);
    expect(next.status).toBe(0);
    expect(next.output.decision).toBe("escalate");
    expect(next.output.reasons[0].code).toBe("pr_merged");
    expect(next.output.backoff.idleStreak).toBe(round);
    const nextState = JSON.parse(readFileSync(f.next, "utf8"));
    expect(nextState.version).toBe(2);
    expect(nextState.totalTicks).toBe(round + 1);
    expect(nextState.lastGoodSnapshot).toBe(`${f.next.replace(/\.json$/, "")}.snapshot.json`);
  }
  expect(readFileSync(`${f.next.replace(/\.json$/, "")}.snapshot.json`)).toEqual(
    readFileSync(snapshot),
  );
  expect(existsSync(`${f.next}.lock`)).toBe(false);
});

test("malformed and foreign state fail before reducing or replacing bytes", () => {
  const f = fixture();
  writeFileSync(f.next, "bad JSON");
  const next = invoke(f.next);
  expect(next.status).toBe(3);
  expect(next.output.errors[0].code).toBe("state_malformed");
  expect(readFileSync(f.next, "utf8")).toBe("bad JSON");
  expect(existsSync(`${f.next}.lock`)).toBe(false);
  writeFileSync(f.next, '{"version":2,"repo":"Other/repo","number":165}');
  const foreign = invoke(f.next);
  expect(foreign.status).toBe(3);
  expect(foreign.output.errors[0].code).toBe("slot_mismatch");
});

test("a live slot lock refuses the tick without touching state", () => {
  const f = fixture();
  mkdirSync(`${f.next}.lock`);
  writeFileSync(join(`${f.next}.lock`, "pid"), `${process.pid}\n`);
  writeFileSync(join(`${f.next}.lock`, "since"), `${Math.floor(Date.now() / 1000)}\n`);
  const next = invoke(f.next);
  expect(next.status).toBe(75);
  expect(next.output.errors[0].code).toBe("locked");
  expect(existsSync(f.next)).toBe(false);
});
