import { afterEach, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, readlinkSync, rmSync } from "node:fs";
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

function hiddenScript(readyPath: string): string {
  return [
    'import { dlopen } from "bun:ffi";',
    'const libc = dlopen("libc.so.6", { prctl: { args: ["i32", "i32", "i32", "i32", "i32"], returns: "i32" } });',
    'if (libc.symbols.prctl(4, 0, 0, 0, 0) !== 0) throw new Error("PR_SET_DUMPABLE failed");',
    `await Bun.write(${JSON.stringify(readyPath)}, String(process.pid));`,
    "await Bun.sleep(30_000);",
  ].join("\n");
}

function hiddenProcess(readyPath: string, cwd: string): Bun.Subprocess {
  return Bun.spawn([process.execPath, "-e", hiddenScript(readyPath)], {
    cwd,
    detached: true,
    stdout: "ignore",
    stderr: "ignore",
  });
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

test("an unrelated unreadable Linux cwd does not block inventory or empty verification", async () => {
  if (process.platform !== "linux" || process.getuid?.() === 0) return;
  const root = mkdtempSync(join(tmpdir(), "toolu-workload-hidden-unrelated-"));
  const outside = mkdtempSync(join(tmpdir(), "toolu-workload-hidden-outside-"));
  temporary.push(root, outside);
  const child = hiddenProcess(join(outside, "ready.pid"), outside);
  groups.push(child.pid);
  await waitFor(join(outside, "ready.pid"));
  expect(() => readlinkSync(`/proc/${child.pid}/cwd`)).toThrow(/EACCES/);

  expect(await ownedWorkload(root)).toEqual([]);
  expect(await verifyOwnedExit([])).toBe(true);
});

test("a known descendant remains owned when its Linux cwd becomes unreadable", async () => {
  if (process.platform !== "linux" || process.getuid?.() === 0) return;
  const root = mkdtempSync(join(tmpdir(), "toolu-workload-hidden-owned-"));
  const outside = mkdtempSync(join(tmpdir(), "toolu-workload-hidden-child-"));
  temporary.push(root, outside);
  const ready = join(outside, "ready.pid");
  const parentScript = [
    `const child = Bun.spawn([process.execPath, "-e", ${JSON.stringify(hiddenScript(ready))}], { cwd: ${JSON.stringify(outside)}, stdout: "ignore", stderr: "ignore" });`,
    "await child.exited;",
  ].join("\n");
  const parent = Bun.spawn([process.execPath, "-e", parentScript], {
    cwd: root,
    detached: true,
    stdout: "ignore",
    stderr: "ignore",
  });
  groups.push(parent.pid);
  const childPid = await waitFor(ready);
  expect(() => readlinkSync(`/proc/${childPid}/cwd`)).toThrow(/EACCES/);

  const records = await ownedWorkload(root);
  expect(records.map((record) => record.pid)).toContain(parent.pid);
  expect(records.map((record) => record.pid)).toContain(childPid);
  expect(await verifyOwnedExit(records)).toBe(false);
});

test("an unreadable Linux cwd in an owned process group keeps ownership uncertain", async () => {
  if (process.platform !== "linux" || process.getuid?.() === 0) return;
  const root = mkdtempSync(join(tmpdir(), "toolu-workload-hidden-group-"));
  const outside = mkdtempSync(join(tmpdir(), "toolu-workload-hidden-group-outside-"));
  temporary.push(root, outside);
  const ready = join(outside, "ready.pid");
  // A dedicated group leader outside the worktree holds the owned process and an
  // unrelated sibling whose cwd is unreadable, so the group never includes the runner.
  const leaderScript = [
    `const owner = Bun.spawn([process.execPath, "-e", "await Bun.sleep(30_000)"], { cwd: ${JSON.stringify(root)}, stdout: "ignore", stderr: "ignore" });`,
    `const hidden = Bun.spawn([process.execPath, "-e", ${JSON.stringify(hiddenScript(ready))}], { cwd: ${JSON.stringify(outside)}, stdout: "ignore", stderr: "ignore" });`,
    "await Promise.all([owner.exited, hidden.exited]);",
  ].join("\n");
  const leader = Bun.spawn([process.execPath, "-e", leaderScript], {
    cwd: outside,
    detached: true,
    stdout: "ignore",
    stderr: "ignore",
  });
  groups.push(leader.pid);
  const hiddenPid = await waitFor(ready);
  expect(() => readlinkSync(`/proc/${hiddenPid}/cwd`)).toThrow(/EACCES/);

  await expect(ownedWorkload(root)).rejects.toThrow("cannot determine worktree ownership");
});
