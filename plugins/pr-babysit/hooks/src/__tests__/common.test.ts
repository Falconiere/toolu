import { afterEach, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
  first.acquire();
  expect(readFileSync(join(lockPath, "pid"), "utf8").trim()).toBe(String(process.pid));
  first.release();
});
