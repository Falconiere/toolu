import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../../../../..");
const snapshot = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/snapshots/toolu-165.json",
);
const nextEntry = join(root, "plugins/pr-babysit/hooks/src/babysit-tick.ts");
const dirs: string[] = [];

function args(state: string, extra: string[] = []): string[] {
  return [
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
  ];
}

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
  const run = spawnSync(process.execPath, args(state, extra), { encoding: "utf8" });
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

test("racing ticks count only completed writers and leave a valid slot", async () => {
  const f = fixture();
  const runs = [0, 1].map(() =>
    Bun.spawn([process.execPath, ...args(f.next)], { stdout: "pipe", stderr: "pipe" }),
  );
  const results = await Promise.all(
    runs.map(async (run) => ({
      status: await run.exited,
      output: JSON.parse(await new Response(run.stdout).text()) as Record<string, any>,
    })),
  );
  for (const result of results) {
    expect([0, 75]).toContain(result.status);
    if (result.status === 75) expect(result.output.errors[0].code).toBe("locked");
  }
  const winners = results.filter((result) => result.status === 0).length;
  expect(winners).toBeGreaterThanOrEqual(1);
  expect(JSON.parse(readFileSync(f.next, "utf8")).totalTicks).toBe(winners);
  expect(existsSync(`${f.next}.lock`)).toBe(false);
  expect(readdirSync(f.dir).some((name) => name.includes(".tmp."))).toBe(false);
});

test("interrupted ticks leave either the old or complete new state", async () => {
  const f = fixture();
  expect(invoke(f.next).status).toBe(0);
  const oldState = readFileSync(f.next, "utf8");
  const secondArgs = args(f.next, ["--now", "2026-09-19T12:03:00Z"]);
  const completed = spawnSync(process.execPath, secondArgs, { encoding: "utf8" });
  expect(completed.status).toBe(0);
  const newState = readFileSync(f.next, "utf8");
  for (let i = 1; i <= 12; i += 1) {
    writeFileSync(f.next, oldState);
    const run = Bun.spawn([process.execPath, ...secondArgs], {
      stdout: "ignore",
      stderr: "ignore",
    });
    await Bun.sleep(i * 2);
    run.kill("SIGTERM");
    await run.exited;
    const state = readFileSync(f.next, "utf8");
    expect([oldState, newState]).toContain(state);
    expect(existsSync(`${f.next}.lock`)).toBe(false);
    expect(readdirSync(f.dir).some((name) => name.includes(".tmp."))).toBe(false);
  }
});

test("failed collection preserves the prior tick and stamps lastError with stub gh", () => {
  const f = fixture();
  expect(invoke(f.next).status).toBe(0);
  const stub = join(f.dir, "gh");
  writeFileSync(stub, "#!/bin/sh\nprintf 'connection refused\\n' >&2\nexit 1\n");
  chmodSync(stub, 0o755);
  const run = spawnSync(
    process.execPath,
    [
      nextEntry,
      "--repo",
      "Falconiere/toolu",
      "--pr",
      "165",
      "--state-file",
      f.next,
      "--now",
      "2026-09-19T12:03:00Z",
    ],
    {
      encoding: "utf8",
      env: { ...process.env, PATH: `${f.dir}:${process.env.PATH}`, PB_GH_BACKOFF: "0 0 0" },
    },
  );
  expect(run.status).toBe(3);
  expect(JSON.parse(run.stdout).errors[0].code).toBe("api_error");
  const state = JSON.parse(readFileSync(f.next, "utf8"));
  expect(state.totalTicks).toBe(1);
  expect(state.pr.lastError.code).toBe("api_error");
  expect(state.pr.lastError.at).toBe("2026-09-19T12:03:00Z");
  expect(existsSync(`${f.next}.lock`)).toBe(false);
});
