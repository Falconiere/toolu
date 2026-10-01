import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { atomicWriteJson, errorDocument, exitCode, SlotLock } from "../babysit/common";
import { BabysitError } from "../babysit/common";

const dirs: string[] = [];
function fixtureDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "pr-babysit-common-"));
  dirs.push(dir);
  return dir;
}
afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

test("closed error codes retain their exit statuses and extra fields", () => {
  expect(exitCode("usage")).toBe(2);
  expect(exitCode("duplicate_reply")).toBe(4);
  expect(exitCode("resolve_unconfirmed")).toBe(5);
  expect(exitCode("locked")).toBe(75);
  expect(exitCode("api_error")).toBe(3);
  expect(errorDocument(new BabysitError("api_error", "boom", { attempts: 3 }))).toEqual({
    version: 1,
    errors: [{ code: "api_error", message: "boom", attempts: 3 }],
  });
});

test("atomic JSON write preserves the old bytes on invalid replacement", () => {
  const path = join(fixtureDir(), "state.json");
  writeFileSync(path, '{"keep":true}\n');
  expect(() => atomicWriteJson(path, null, "not json")).toThrow();
  expect(readFileSync(path, "utf8")).toBe('{"keep":true}\n');
  atomicWriteJson(path, { next: true });
  expect(readFileSync(path, "utf8")).toBe('{"next":true}\n');
});

test("slot lock records this process, refuses a live holder, then releases", () => {
  const path = join(fixtureDir(), "state.json");
  const first = new SlotLock(path);
  first.acquire();
  expect(readFileSync(`${path}.lock/pid`, "utf8").trim()).toBe(String(process.pid));
  const second = new SlotLock(path);
  expect(() => second.acquire()).toThrow(BabysitError);
  first.release();
  expect(existsSync(`${path}.lock`)).toBe(false);
});

test("slot lock reclaims a half-written stale directory", () => {
  const path = join(fixtureDir(), "state.json");
  const lockPath = `${path}.lock`;
  const first = new SlotLock(path);
  // A crashed holder may leave a directory without pid/since.
  mkdirSync(lockPath);
  const old = new Date(Date.now() - 2000);
  utimesSync(lockPath, old, old);
  first.acquire();
  expect(readFileSync(join(lockPath, "pid"), "utf8").trim()).toBe(String(process.pid));
  first.release();
});

test("slot lock does not reclaim a freshly created directory before its metadata is written", () => {
  const path = join(fixtureDir(), "state.json");
  const lockPath = `${path}.lock`;
  mkdirSync(lockPath);
  const contender = new SlotLock(path);
  expect(() => contender.acquire()).toThrow(BabysitError);
  expect(existsSync(lockPath)).toBe(true);
  expect(existsSync(join(lockPath, "pid"))).toBe(false);
});

test("slot lock cleans its incomplete directory but leaves a replacement alone", () => {
  const path = join(fixtureDir(), "state.json");
  const lockPath = `${path}.lock`;
  const first = new SlotLock(path);
  first.acquire();
  rmSync(join(lockPath, "pid"));
  first.release();
  expect(existsSync(lockPath)).toBe(false);

  const second = new SlotLock(path);
  second.acquire();
  renameSync(lockPath, `${lockPath}.old`);
  mkdirSync(lockPath);
  writeFileSync(join(lockPath, "pid"), `${process.pid}\n`);
  second.release();
  expect(existsSync(lockPath)).toBe(true);
});

test("slot lock is gone before its SIGTERM handler is removed", () => {
  const dir = fixtureDir();
  const statePath = join(dir, "state.json");
  const script = join(dir, "interrupt-release.ts");
  writeFileSync(
    script,
    `import { existsSync } from "node:fs";
import { SlotLock } from ${JSON.stringify(new URL("../babysit/common.ts", import.meta.url).href)};

const lock = new SlotLock(process.argv[2]!);
lock.acquire();
// A signal with no remaining handler would terminate at this exact boundary.
const off = process.off.bind(process);
process.off = ((event, listener) => {
  const result = off(event, listener);
  if (event === "SIGTERM") process.exit(existsSync(lock.path) ? 42 : 0);
  return result;
}) as typeof process.off;
lock.release();
process.exit(43);
`,
  );
  const run = spawnSync(process.execPath, [script, statePath], { encoding: "utf8" });
  expect(run.status).toBe(0);
  expect(existsSync(`${statePath}.lock`)).toBe(false);
});
