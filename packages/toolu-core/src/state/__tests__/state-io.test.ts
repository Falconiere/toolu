/**
 * state-io (#255): jq-identical serialization and string order, checked
 * against the real `jq` binary; atomic writes and the sidecar lock on real
 * files.
 */
import { expect, test } from "bun:test";
import {
  existsSync,
  readdirSync,
  readFileSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { compareJqStrings, isoSeconds, toJqJson, withLock, writeAtomic } from "../state-io.ts";

const AWKWARD = {
  del: "a\u007fb",
  control: "tab\tnl\ncr\r\u0001\u001f",
  unicode: "é ～ 😀  ",
  quote: 'q"\\',
  empty: {},
  list: [],
  nested: { n: 1.5, big: 12345678901, neg: -3, t: true, f: false, z: null },
};

async function jq(args: string[], input: string): Promise<string> {
  const res = await run(["jq", ...args], { stdin: input });
  expect(res.exitCode).toBe(0);
  return res.stdout;
}

test("toJqJson pretty equals `jq .` byte for byte", async () => {
  const input = JSON.stringify(AWKWARD);
  expect(`${toJqJson(AWKWARD, true)}\n`).toBe(await jq(["."], input));
});

test("toJqJson compact equals `jq -c .` byte for byte", async () => {
  const input = JSON.stringify(AWKWARD);
  expect(`${toJqJson(AWKWARD, false)}\n`).toBe(await jq(["-c", "."], input));
});

test("compareJqStrings orders like jq sort (codepoint, not UTF-16)", async () => {
  const words = ["😀", "～", "b", "B", "", "é", "a\u007f", "a"];
  const sorted = JSON.parse(await jq(["-c", "sort"], JSON.stringify(words)));
  expect([...words].sort(compareJqStrings)).toEqual(sorted);
  // The default JS sort disagrees on this input, which is why the helper exists.
  expect([...words].sort()).not.toEqual(sorted);
});

test("isoSeconds matches `date -u +%Y-%m-%dT%H:%M:%SZ` shape and truncates", () => {
  expect(isoSeconds(new Date("2026-09-28T12:34:56.789Z"))).toBe("2026-09-28T12:34:56Z");
});

test("writeAtomic replaces the file and leaves no temp behind", () => {
  using sb = createSandbox();
  const file = sb.write("state/gate.json", "old\n");
  expect(writeAtomic(file, "new\n")).toBe(true);
  expect(readFileSync(file, "utf8")).toBe("new\n");
  expect(readdirSync(join(sb.project, "state"))).toEqual(["gate.json"]);
});

test("writeAtomic reports failure when the directory is missing", () => {
  using sb = createSandbox();
  const file = join(sb.project, "missing", "gate.json");
  expect(writeAtomic(file, "x")).toBe(false);
  expect(existsSync(file)).toBe(false);
});

test("withLock returns the result and releases the lock, even on throw", () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  expect(withLock(file, () => existsSync(`${file}.lock`))).toBe(true);
  expect(existsSync(`${file}.lock`)).toBe(false);
  expect(() =>
    withLock(file, () => {
      throw new Error("boom");
    }),
  ).toThrow("boom");
  expect(existsSync(`${file}.lock`)).toBe(false);
});

test("withLock breaks a stale lock left by a crashed writer", () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  writeFileSync(`${file}.lock`, "99999 crashed\n");
  const old = new Date(Date.now() - 60_000);
  utimesSync(`${file}.lock`, old, old);
  const warnings: string[] = [];
  const ran = withLock(file, () => true, { warn: (m) => warnings.push(m) });
  expect(ran).toBe(true);
  expect(warnings).toEqual([]);
  expect(existsSync(`${file}.lock`)).toBe(false);
});

test("withLock times out on a live lock, warns, and still runs unlocked", () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  writeFileSync(`${file}.lock`, "1 live\n");
  const warnings: string[] = [];
  const started = Date.now();
  const ran = withLock(file, () => true, { timeoutMs: 100, warn: (m) => warnings.push(m) });
  expect(ran).toBe(true);
  expect(Date.now() - started).toBeGreaterThanOrEqual(100);
  expect(warnings).toEqual([`state: lock ${file}.lock still held; writing without it`]);
  // Someone else's lock is not ours to remove.
  expect(existsSync(`${file}.lock`)).toBe(true);
});

test("writeAtomic creates the file 0600, as bash's mktemp + mv does", () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  expect(writeAtomic(file, "x")).toBe(true);
  expect(statSync(file).mode & 0o777).toBe(0o600);
});

test("withLock breaks a fresh lock whose holder pid is gone", async () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  const done = await run(["bash", "-c", "echo $$"]);
  writeFileSync(`${file}.lock`, `${done.stdout.trim()} crashed-token\n`);
  const warnings: string[] = [];
  const started = Date.now();
  expect(withLock(file, () => true, { warn: (m) => warnings.push(m) })).toBe(true);
  expect(Date.now() - started).toBeLessThan(1000);
  expect(warnings).toEqual([]);
});

test("withLock breaks a live holder's lock once it is older than staleMs (2 s default)", () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  writeFileSync(`${file}.lock`, `${String(process.pid)} hung-token\n`);
  const old = new Date(Date.now() - 3000);
  utimesSync(`${file}.lock`, old, old);
  const warnings: string[] = [];
  expect(withLock(file, () => true, { warn: (m) => warnings.push(m) })).toBe(true);
  expect(warnings).toEqual([]);
  expect(existsSync(`${file}.lock`)).toBe(false);
});

test("withLock never releases a lock another holder took over", () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  withLock(file, () => {
    // Someone judged ours stale and took the lock while fn ran.
    writeFileSync(`${file}.lock`, "4242 their-token\n");
  });
  expect(readFileSync(`${file}.lock`, "utf8")).toBe("4242 their-token\n");
});

test("breaking a stale lock leaves no claimed .broken file behind", () => {
  using sb = createSandbox();
  const file = sb.path("gate.json");
  writeFileSync(`${file}.lock`, "99999 crashed\n");
  const old = new Date(Date.now() - 60_000);
  utimesSync(`${file}.lock`, old, old);
  withLock(file, () => true);
  expect(readdirSync(sb.project).filter((name) => name.includes(".lock"))).toEqual([]);
});
