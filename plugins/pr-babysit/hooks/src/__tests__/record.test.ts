import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../../../../..");
const fixture = join(root, "plugins/pr-babysit/scripts/__tests__/fixtures/states/toolu-165-initial.json");
const entry = join(root, "plugins/pr-babysit/hooks/src/babysit-record.ts");
const baseline = join(root, "plugins/pr-babysit/scripts/record.sh");
const dirs: string[] = [];

function files(): { old: string; next: string } {
  const dir = mkdtempSync(join(tmpdir(), "babysit-record-"));
  dirs.push(dir);
  const old = join(dir, "old.json");
  const next = join(dir, "next.json");
  copyFileSync(fixture, old);
  copyFileSync(fixture, next);
  return { old, next };
}

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function invoke(command: string, file: string, args: string[]): { status: number | null; output: Record<string, unknown>; state: Record<string, unknown> } {
  const run = spawnSync(command === "bash" ? "bash" : process.execPath, [command === "bash" ? baseline : entry, ...args, "--state-file", file], { encoding: "utf8" });
  return { status: run.status, output: JSON.parse(run.stdout), state: JSON.parse(readFileSync(file, "utf8")) };
}

function stripTimes(value: Record<string, unknown>): Record<string, unknown> {
  const clone = structuredClone(value);
  const actions = clone.actions as Record<string, Record<string, Record<string, unknown>>> | undefined;
  if (actions?.flagged) for (const item of Object.values(actions.flagged)) delete item.at;
  const lastRound = clone.lastRound as Record<string, unknown> | undefined;
  if (lastRound) delete lastRound.at;
  delete clone.statusChangedAt;
  delete clone.at;
  return clone;
}

test("round rotates real captured finding keys and caps attempts like bash", () => {
  const paths = files();
  for (const args of [
    ["round", "--had-rejection", "true", "--fix-pushed"],
    ["round", "--had-rejection", "false"],
  ]) {
    const old = invoke("bash", paths.old, args);
    const next = invoke("bun", paths.next, args);
    expect(next.status).toBe(old.status);
    expect(stripTimes(next.output)).toEqual(stripTimes(old.output));
    expect(stripTimes(next.state)).toEqual(stripTimes(old.state));
  }
});

test("flag and status update the captured state like bash", () => {
  const paths = files();
  for (const args of [["flag-injection", "--thread", "PRRT_example"], ["status", "--status", "complete"]]) {
    const old = invoke("bash", paths.old, args);
    const next = invoke("bun", paths.next, args);
    expect(next.status).toBe(old.status);
    expect(stripTimes(next.output)).toEqual(stripTimes(old.output));
    expect(stripTimes(next.state)).toEqual(stripTimes(old.state));
  }
});

test("missing state fails without creating a file", () => {
  const paths = files();
  rmSync(paths.old);
  rmSync(paths.next);
  const old = spawnSync("bash", [baseline, "status", "--state-file", paths.old, "--status", "complete"], { encoding: "utf8" });
  const next = spawnSync(process.execPath, [entry, "status", "--state-file", paths.next, "--status", "complete"], { encoding: "utf8" });
  expect(next.status).toBe(old.status);
  expect(JSON.parse(next.stdout).errors[0].code).toBe(JSON.parse(old.stdout).errors[0].code);
});
