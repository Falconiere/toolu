import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  DEFAULT_MAX_OUTPUT_BYTES,
  DEFAULT_TIMEOUT_MS,
  processGroupAlive,
  runCommand,
} from "../process.ts";

const temporaryDirectories: string[] = [];

function temporaryDirectory(): string {
  const path = mkdtempSync(join(tmpdir(), "toolu-process-"));
  temporaryDirectories.push(path);
  return path;
}

afterEach(() => {
  for (const path of temporaryDirectories.splice(0)) {
    rmSync(path, { recursive: true, force: true });
  }
});

function running(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function waitForFile(path: string): Promise<number> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (existsSync(path)) return Number(readFileSync(path, "utf8"));
    await Bun.sleep(20);
  }
  throw new Error(`process did not write ${path}`);
}

async function waitForExit(pid: number): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (!running(pid)) return;
    await Bun.sleep(20);
  }
  throw new Error(`process ${pid} is still running`);
}

describe("runCommand", () => {
  test("captures real output, stdin, cwd, environment and native status", async () => {
    const cwd = temporaryDirectory();
    let spawnedPid = 0;
    const script = [
      "const input = await Bun.stdin.text();",
      "process.stdout.write(`${process.cwd()}|${process.env.TOOLU_PROCESS_TEST}|${input}`);",
      'process.stderr.write("problem\\n");',
      "process.exit(7);",
    ].join("\n");
    const result = await runCommand([process.execPath, "-e", script], {
      cwd,
      env: { ...process.env, TOOLU_PROCESS_TEST: "present" },
      stdin: "payload",
      onSpawn(pid) {
        spawnedPid = pid;
        expect(processGroupAlive(pid)).toBe(true);
      },
    });

    expect(result).toMatchObject({
      stdout: `${cwd}|present|payload`,
      stderr: "problem\n",
      exitCode: 7,
      timedOut: false,
      cancelled: false,
      truncated: false,
    });
    expect(result.durationMs).toBeGreaterThan(0);
    expect(spawnedPid).toBeGreaterThan(0);
    expect(processGroupAlive(spawnedPid)).toBe(false);
  });

  test("does not feed stdin until the spawn callback persists ownership", async () => {
    const dir = temporaryDirectory();
    const readyFile = join(dir, "owned");
    const script = [
      "const input = await Bun.stdin.text();",
      `const owned = await Bun.file(${JSON.stringify(readyFile)}).exists();`,
      "process.stdout.write(`${owned}:${input}`);",
    ].join("\n");
    const result = await runCommand([process.execPath, "-e", script], {
      stdin: "toolu-go\n",
      async onSpawn() {
        await Bun.sleep(100);
        await Bun.write(readyFile, "yes");
      },
    });

    expect(result.stdout).toBe("true:toolu-go\n");
  });

  test("a blocked spawn callback remains inside the command deadline", async () => {
    const started = performance.now();
    const result = await runCommand([process.execPath, "-e", "await Bun.sleep(30_000)"], {
      timeoutMs: 300,
      onSpawn: () => new Promise<void>(() => undefined),
    });

    expect(result.timedOut).toBe(true);
    expect(performance.now() - started).toBeLessThan(2500);
  });

  test("a failed spawn callback tears down the owned process group", async () => {
    let pid = 0;
    const error = await runCommand([process.execPath, "-e", "await Bun.sleep(30_000)"], {
      onSpawn(spawnedPid) {
        pid = spawnedPid;
        throw new Error("lease write failed");
      },
    }).then(
      () => null,
      (caught: unknown) => caught,
    );

    expect(String(error)).toContain("lease write failed");
    expect(pid).toBeGreaterThan(0);
    expect(processGroupAlive(pid)).toBe(false);
  });

  test("drains large stderr while retaining only the aggregate byte bound", async () => {
    const result = await runCommand(
      [
        process.execPath,
        "-e",
        'process.stderr.write("e".repeat(4_000_000)); process.stdout.write("done");',
      ],
      { maxOutputBytes: 4096 },
    );

    expect(result.exitCode).toBe(0);
    expect(Buffer.byteLength(result.stdout) + Buffer.byteLength(result.stderr)).toBeLessThanOrEqual(
      4096,
    );
    expect(result.truncated).toBe(true);
    expect(result.timedOut).toBe(false);
  });

  test("a timeout terminates a child and grandchild process group", async () => {
    const dir = temporaryDirectory();
    const childPidFile = join(dir, "child.pid");
    const grandchildPidFile = join(dir, "grandchild.pid");
    const grandchild = "process.on('SIGTERM', () => {}); await Bun.sleep(30_000);";
    const child = [
      "process.on('SIGTERM', () => {});",
      `const grandchild = Bun.spawn([process.execPath, "-e", ${JSON.stringify(grandchild)}], { stdout: "inherit", stderr: "inherit" });`,
      `await Bun.write(${JSON.stringify(grandchildPidFile)}, String(grandchild.pid));`,
      "await grandchild.exited;",
    ].join("\n");
    const parent = [
      "process.on('SIGTERM', () => {});",
      `const child = Bun.spawn([process.execPath, "-e", ${JSON.stringify(child)}], { stdout: "inherit", stderr: "inherit" });`,
      `await Bun.write(${JSON.stringify(childPidFile)}, String(child.pid));`,
      "await child.exited;",
    ].join("\n");

    const result = await runCommand([process.execPath, "-e", parent], {
      timeoutMs: 800,
    });
    const childPid = await waitForFile(childPidFile);
    const grandchildPid = await waitForFile(grandchildPidFile);

    expect(result.timedOut).toBe(true);
    expect(result.cancelled).toBe(false);
    expect(result.exitCode).toBe(137);
    expect(result.durationMs).toBeLessThan(4000);
    await Promise.all([waitForExit(childPid), waitForExit(grandchildPid)]);
  });

  test("the deadline includes a descendant retaining output after its parent exits", async () => {
    const dir = temporaryDirectory();
    const pidFile = join(dir, "descendant.pid");
    const child = [
      "const child = Bun.spawn([process.execPath, '-e', 'await Bun.sleep(30_000)'], { stdout: 'inherit', stderr: 'inherit' });",
      `await Bun.write(${JSON.stringify(pidFile)}, String(child.pid));`,
      "child.unref();",
    ].join("\n");

    const result = await runCommand([process.execPath, "-e", child], {
      timeoutMs: 400,
    });
    const pid = await waitForFile(pidFile);

    expect(result).toMatchObject({
      exitCode: 0,
      timedOut: true,
      cancelled: false,
    });
    expect(result.durationMs).toBeLessThan(3000);
    await waitForExit(pid);
  });

  test("normal completion waits for a background group member with redirected output", async () => {
    const started = performance.now();
    const result = await runCommand(["sh", "-c", "sleep 0.35 >/dev/null 2>&1 &"], {
      timeoutMs: 2000,
    });

    expect(result).toMatchObject({
      exitCode: 0,
      timedOut: false,
      cancelled: false,
    });
    expect(performance.now() - started).toBeGreaterThan(250);
  });

  test("a redirected background descendant is killed when the group deadline expires", async () => {
    const dir = temporaryDirectory();
    const pidFile = join(dir, "background.pid");
    const result = await runCommand(
      ["sh", "-c", `sleep 30 >/dev/null 2>&1 & echo $! > '${pidFile}'`],
      { timeoutMs: 400 },
    );
    const pid = await waitForFile(pidFile);

    expect(result).toMatchObject({
      exitCode: 0,
      timedOut: true,
      cancelled: false,
    });
    await waitForExit(pid);
  });

  test("AbortSignal cancellation returns only after the command is stopped", async () => {
    const controller = new AbortController();
    const command = runCommand(
      [
        process.execPath,
        "-e",
        "process.on('SIGTERM', () => {}); process.stdout.write('started'); await Bun.sleep(30_000);",
      ],
      { signal: controller.signal, timeoutMs: 10_000 },
    );
    await Bun.sleep(100);
    controller.abort();
    const result = await command;

    expect(result).toMatchObject({
      stdout: "started",
      exitCode: 137,
      timedOut: false,
      cancelled: true,
      truncated: false,
    });
    expect(result.durationMs).toBeLessThan(3000);
  });

  test("a parent signal and explicit exit kill the command and its descendants", async () => {
    const dir = temporaryDirectory();
    const runnerFile = join(dir, "runner.ts");
    const childPidFile = join(dir, "child.pid");
    const grandchildPidFile = join(dir, "grandchild.pid");
    const processModule = new URL("../process.ts", import.meta.url).href;
    const grandchild = "process.on('SIGTERM', () => {}); await Bun.sleep(30_000);";
    const child = [
      "process.on('SIGTERM', () => {});",
      `const grandchild = Bun.spawn([process.execPath, "-e", ${JSON.stringify(grandchild)}], { stdin: "ignore", stdout: "ignore", stderr: "ignore" });`,
      `await Bun.write(${JSON.stringify(grandchildPidFile)}, String(grandchild.pid));`,
      "await Bun.sleep(30_000);",
    ].join("\n");
    await Bun.write(
      runnerFile,
      [
        `import { runCommand } from ${JSON.stringify(processModule)};`,
        'process.once("SIGTERM", () => process.exit(143));',
        `await runCommand([process.execPath, "-e", ${JSON.stringify(child)}], {`,
        "  timeoutMs: 30_000,",
        `  onSpawn: (pid) => Bun.write(${JSON.stringify(childPidFile)}, String(pid)),`,
        "});",
      ].join("\n"),
    );
    const runner = Bun.spawn([process.execPath, runnerFile], {
      stdin: "ignore",
      stdout: "ignore",
      stderr: "ignore",
    });
    const [childPid, grandchildPid] = await Promise.all([
      waitForFile(childPidFile),
      waitForFile(grandchildPidFile),
    ]);

    runner.kill("SIGTERM");
    expect(await runner.exited).toBe(143);
    await Promise.all([waitForExit(childPid), waitForExit(grandchildPid)]);
    expect(processGroupAlive(childPid)).toBe(false);
  });

  test("a pre-aborted signal does not spawn", async () => {
    const controller = new AbortController();
    controller.abort();
    expect(
      await runCommand([process.execPath, "-e", 'process.stdout.write("unreachable")'], {
        signal: controller.signal,
      }),
    ).toMatchObject({
      stdout: "",
      stderr: "",
      exitCode: 130,
      timedOut: false,
      cancelled: true,
      truncated: false,
    });
  });

  test("rejects invalid execution bounds and empty argv", async () => {
    expect(DEFAULT_TIMEOUT_MS).toBeGreaterThan(0);
    expect(DEFAULT_MAX_OUTPUT_BYTES).toBeGreaterThan(0);
    expect(() => runCommand([])).toThrow("argv must not be empty");
    expect(() => runCommand(["true"], { timeoutMs: Number.NaN })).toThrow(
      "timeoutMs must be a positive finite number",
    );
    expect(() => runCommand(["true"], { timeoutMs: 0 })).toThrow(
      "timeoutMs must be a positive finite number",
    );
    expect(() => runCommand(["true"], { maxOutputBytes: -1 })).toThrow(
      "maxOutputBytes must be a non-negative safe integer",
    );
  });
});
