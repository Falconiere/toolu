/**
 * AC-8 (#256): the step-check runner on real processes, and the evidence
 * encoding. A check that overruns
 * PLAN_LEDGER_STEP_TIMEOUT is killed with its whole process group. That holds
 * even when no `timeout` binary is reachable (D1). A check reading stdin sees
 * EOF, and evidence handles NUL, long and invalid-UTF-8 output.
 */
import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { bindWorktree } from "../../resources/binding.ts";
import {
  acquireLease,
  readResourceState,
  releaseLease,
  writeJsonAtomic,
} from "../../resources/resources.ts";
import { TIMEOUT_EXIT, evidenceOf, parseTimeout, runCheck, stepEvidence } from "../ledger-check.ts";

/** A PATH holding only bash, sleep and cat: no `timeout`/`gtimeout` binary is reachable. */
function barePath(sb: Sandbox): string {
  const bin = join(sb.root, "bin");
  mkdirSync(bin, { recursive: true });
  for (const tool of ["bash", "sleep", "cat"]) {
    const target = Bun.which(tool);
    if (target === null) throw new Error(`${tool} not on PATH`);
    symlinkSync(target, join(bin, tool));
  }
  return bin;
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function check(sb: Sandbox, script: string, timeout: string, path = process.env.PATH ?? "") {
  const outFile = join(sb.root, "out.txt");
  const code = await runCheck({
    check: script,
    cwd: sb.project,
    env: { PATH: path },
    outFile,
    timeout,
  });
  return { code, output: readFileSync(outFile) };
}

describe("runCheck on real processes", () => {
  test.concurrent("stdout and stderr interleave into one file, exit code kept", async () => {
    using sb = createSandbox();
    const res = await check(sb, "echo out; echo err >&2; echo out2; exit 3", "1800");
    expect(res).toEqual({ code: 3, output: Buffer.from("out\nerr\nout2\n") });
  });

  test.concurrent("a check that reads stdin sees EOF instead of blocking", async () => {
    using sb = createSandbox();
    const res = await check(sb, "cat; echo done", "5");
    expect(res).toEqual({ code: 0, output: Buffer.from("done\n") });
  });

  test.concurrent("an overrun check is killed with its background child, exit 124", async () => {
    using sb = createSandbox();
    const pidFile = join(sb.root, "child.pid");
    const started = Date.now();
    const res = await check(sb, `sleep 30 & echo $! > '${pidFile}'; sleep 30`, "1", barePath(sb));
    expect(res.code).toBe(124);
    expect(Date.now() - started).toBeLessThan(5000);
    const child = Number(readFileSync(pidFile, "utf8").trim());
    await Bun.sleep(100);
    expect(alive(child)).toBe(false);
  });

  test.concurrent("a check that ignores SIGTERM is still stopped", async () => {
    using sb = createSandbox();
    const started = Date.now();
    const res = await check(sb, "trap '' TERM; sleep 30", "0.5", barePath(sb));
    expect(res.code).toBe(124);
    expect(Date.now() - started).toBeLessThan(6000);
  });

  test.concurrent("PLAN_LEDGER_STEP_TIMEOUT=0 disables the bound", async () => {
    using sb = createSandbox();
    expect((await check(sb, "sleep 1.2; exit 0", "0")).code).toBe(0);
  });

  test.concurrent("death by signal reports 128+N", async () => {
    using sb = createSandbox();
    expect((await check(sb, "kill -9 $$", "10")).code).toBe(137);
  });

  test.concurrent("an unparseable bound is a red 125 with the reason as output", async () => {
    using sb = createSandbox();
    const res = await check(sb, "true", "soon");
    expect(res.code).toBe(125);
    expect(res.output.toString()).toContain("invalid PLAN_LEDGER_STEP_TIMEOUT 'soon'");
  });
});

describe("resource-bound ledger checks", () => {
  test("runs under job admission while preserving combined output and status", async () => {
    using sb = createSandbox({ git: true });
    const root = join(sb.root, "resources");
    await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
    bindWorktree(root, sb.project, "issue-376", join(sb.root, "epic"));

    const res = await check(sb, "echo out; echo err >&2; exit 3", "5");

    expect(res).toEqual({ code: 3, output: Buffer.from("out\nerr\n") });
    expect(readResourceState(root).leases).toHaveLength(0);
  });

  test("truncated managed check output fails closed", async () => {
    using sb = createSandbox({ git: true });
    const root = join(sb.root, "resources");
    await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
    bindWorktree(root, sb.project, "issue-376", join(sb.root, "epic"));

    const res = await check(
      sb,
      `${process.execPath} -e 'process.stdout.write("x".repeat(1100000))'`,
      "5",
    );

    expect(res.code).toBe(125);
    expect(
      res.output.toString().endsWith("plan-ledger: check output exceeded capture limit\n"),
    ).toBe(true);
    expect(readResourceState(root).leases).toHaveLength(0);
  });

  test("capacity refusal happens before the mandatory check starts", async () => {
    using sb = createSandbox({ git: true });
    const root = join(sb.root, "resources");
    const stateDir = join(sb.root, "epic");
    const marker = join(sb.root, "started");
    await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
    bindWorktree(root, sb.project, "issue-376", stateDir);
    const occupied = await acquireLease(root, {
      type: "job",
      key: "occupied",
      stateDir,
      worktree: sb.project,
    });

    await expect(check(sb, `echo started > '${marker}'`, "5")).rejects.toThrow("capacity");
    expect(existsSync(marker)).toBe(false);
    await releaseLease(root, occupied.token);
  });

  test("timeout kills a managed check's redirected background descendant", async () => {
    using sb = createSandbox({ git: true });
    const root = join(sb.root, "resources");
    const pidFile = join(sb.root, "managed-child.pid");
    await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
    bindWorktree(root, sb.project, "issue-376", join(sb.root, "epic"));

    const res = await check(sb, `sleep 30 >/dev/null 2>&1 & echo $! > '${pidFile}'`, "0.4");
    const child = Number(readFileSync(pidFile, "utf8").trim());

    expect(res.code).toBe(TIMEOUT_EXIT);
    await Bun.sleep(100);
    expect(alive(child)).toBe(false);
    expect(readResourceState(root).leases).toHaveLength(0);
  });
});

test("parseTimeout reads GNU timeout's decimal durations", () => {
  const cases: [string, number | undefined][] = [
    ["1800", 1800],
    ["0", 0],
    ["1.5", 1.5],
    [".5", 0.5],
    ["2m", 120],
    ["1h", 3600],
    ["1d", 86400],
    ["5s", 5],
    ["00", 0],
    ["1e3", 1000],
    ["+5", 5],
    [" 5", 5],
    ["2.5e1m", 1500],
    ["x", undefined],
    ["-1", undefined],
    ["5 ", undefined],
    ["0x10", undefined],
    ["", undefined],
  ];
  expect(cases.map(([text]) => [text, parseTimeout(text)])).toEqual(cases);
});

test.concurrent("a bound past setTimeout's range still lets the check finish", async () => {
  using sb = createSandbox();
  expect(await check(sb, "sleep 0.2; exit 4", "99999999")).toEqual({
    code: 4,
    output: Buffer.from(""),
  });
});

test.concurrent("a runner killed mid-check takes the check's process group with it", async () => {
  using sb = createSandbox();
  const pidFile = join(sb.root, "check.pid");
  const runner = Bun.spawn(
    ["bun", "run", join(import.meta.dir, "run-check-runner.ts"), sb.project, pidFile],
    {
      stdout: "ignore",
      stderr: "ignore",
      env: { PATH: process.env.PATH ?? "", HOME: sb.home },
    },
  );
  let pid = 0;
  for (let i = 0; i < 100 && pid === 0; i += 1) {
    await Bun.sleep(50);
    pid = Number(existsSync(pidFile) ? readFileSync(pidFile, "utf8").trim() : 0);
  }
  expect(pid).toBeGreaterThan(0);
  expect(alive(pid)).toBe(true);
  runner.kill("SIGTERM");
  expect(await runner.exited).not.toBe(0);
  await Bun.sleep(200);
  expect(alive(pid)).toBe(false);
});

const eleven = Array.from({ length: 12 }, (_, i) => `line ${i}`).join("\n");
const OUTPUTS: Record<string, Uint8Array> = {
  empty: new Uint8Array(),
  "twelve lines with trailing newlines": Buffer.from(`${eleven}\n\n\n`),
  "NUL bytes": Buffer.from("a\0b\n\0c\0\n"),
  "CRLF and DEL": Buffer.from("x\r\ny\u007f\r\n"),
  "3000-byte line": Buffer.from("z".repeat(3000)),
  "cut inside a multibyte char": Buffer.from(`${"a".repeat(1999)}é tail`),
  "invalid UTF-8": Buffer.from([0x61, 0xff, 0x62, 0xc3, 0x28, 0xe2, 0x82, 0x0a, 0xf0, 0x9f, 0x98]),
  "only newlines": Buffer.from("\n\n\n"),
};

describe("evidenceOf handles raw output", () => {
  for (const [name, bytes] of Object.entries(OUTPUTS)) {
    test.concurrent(name, () => {
      expect(typeof evidenceOf(bytes)).toBe("string");
      expect(evidenceOf(bytes).length).toBeLessThanOrEqual(2000);
    });
  }
});

describe("stepEvidence includes timeout context", () => {
  for (const [name, bytes] of Object.entries(OUTPUTS)) {
    test.concurrent(name, () => {
      expect(stepEvidence(124, bytes, "7")).toContain("timed out after 7s");
    });
  }
});
