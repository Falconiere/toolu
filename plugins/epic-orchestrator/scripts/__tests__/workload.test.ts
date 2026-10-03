import { afterEach, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ownedWorkload, verifyOwnedExit } from "../workload.ts";

const temporary: string[] = [];
const groups: number[] = [];

afterEach(() => {
  for (const group of groups.splice(0)) {
    try {
      process.kill(-group, "SIGKILL");
    } catch {}
  }
  for (const path of temporary.splice(0)) rmSync(path, { recursive: true, force: true });
});

async function waitFor(path: string): Promise<number> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (existsSync(path)) return Number(readFileSync(path, "utf8"));
    await Bun.sleep(20);
  }
  throw new Error(`missing process fixture ${path}`);
}

test("finds worktree processes and descendants after they change cwd", async () => {
  if (process.platform !== "linux" && process.platform !== "darwin") return;
  const root = mkdtempSync(join(tmpdir(), "toolu-workload-"));
  const outside = mkdtempSync(join(tmpdir(), "toolu-workload-outside-"));
  temporary.push(root, outside);
  const pidFile = join(root, "grandchild.pid");
  const grandchild = `process.chdir(${JSON.stringify(outside)}); await Bun.write(${JSON.stringify(pidFile)}, String(process.pid)); await Bun.sleep(30_000);`;
  const child = [
    `const grandchild = Bun.spawn([process.execPath, "-e", ${JSON.stringify(grandchild)}], { stdout: "ignore", stderr: "ignore" });`,
    "await grandchild.exited;",
  ].join("\n");
  const processUnderTest = Bun.spawn([process.execPath, "-e", child], {
    cwd: root,
    stdout: "ignore",
    stderr: "ignore",
    detached: true,
  });
  groups.push(processUnderTest.pid);
  const grandchildPid = await waitFor(pidFile);

  const records = await ownedWorkload(root);

  expect(records.map((record) => record.pid)).toContain(processUnderTest.pid);
  expect(records.map((record) => record.pid)).toContain(grandchildPid);
  expect(await verifyOwnedExit(records)).toBe(false);
  process.kill(-processUnderTest.pid, "SIGKILL");
  await processUnderTest.exited;
  for (let attempt = 0; attempt < 40 && !(await verifyOwnedExit(records)); attempt += 1) {
    await Bun.sleep(25);
  }
  expect(await verifyOwnedExit(records)).toBe(true);
});

test("excludes the caller and an explicitly supplied shell PID", async () => {
  if (process.platform !== "linux" && process.platform !== "darwin") return;
  const root = mkdtempSync(join(tmpdir(), "toolu-workload-exclude-"));
  temporary.push(root);
  const shell = Bun.spawn([process.execPath, "-e", "await Bun.sleep(30_000)"], {
    cwd: root,
    detached: true,
    stdout: "ignore",
    stderr: "ignore",
  });
  groups.push(shell.pid);

  const records = await ownedWorkload(root, shell.pid);

  expect(records.some((record) => record.pid === process.pid)).toBe(false);
  expect(records.some((record) => record.pid === shell.pid)).toBe(false);
});

test("Linux inventory reads current-user processes without requiring root", async () => {
  if (process.platform !== "linux") return;
  const root = mkdtempSync(join(tmpdir(), "toolu-workload-linux-"));
  temporary.push(root);
  expect(await ownedWorkload(root)).toEqual([]);
});
