/** jq-compatible serialization, atomic writes and real lock files from shared cases. */
import { expect, test } from "bun:test";
import {
  existsSync,
  readdirSync,
  readFileSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { materializeCaseValue, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { compareJqStrings, isoSeconds, toJqJson, withLock, writeAtomic } from "../state-io.ts";

const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("io"),
  scenario: z.enum([
    "jq-json",
    "jq-sort",
    "iso",
    "atomic-replace",
    "atomic-missing",
    "lock-result",
    "stale-lock",
    "live-lock",
    "atomic-mode",
    "dead-pid-lock",
    "stale-own-pid",
    "lock-takeover",
    "stale-no-broken",
  ]),
  value: z.json().optional(),
  pretty: z.boolean().optional(),
  jqArgs: z.array(z.string()).optional(),
  words: z.array(z.string()).optional(),
  jsSortDiffers: z.boolean().optional(),
  input: z.string().optional(),
  expected: z.string().optional(),
  file: z.string().optional(),
  before: z.string().optional(),
  after: z.string().optional(),
  remaining: z.array(z.string()).optional(),
  body: z.string().optional(),
  error: z.string().optional(),
  lock: z.string().optional(),
  ageMs: z.number().int().optional(),
  expectedWarnings: z.array(z.string()).optional(),
  timeoutMs: z.number().int().optional(),
  warning: z.json().optional(),
  mode: z.number().int().optional(),
  lockSuffix: z.string().optional(),
  maxMs: z.number().int().optional(),
  otherLock: z.string().optional(),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/state/cases.json"))
  .filter((raw) => raw.kind === "io")
  .map((raw) => CaseSchema.parse(raw));
const str = (value: unknown) => z.string().parse(value);
const num = (value: unknown) => z.number().parse(value);

async function jq(args: string[], input: string): Promise<string> {
  const res = await run(["jq", ...args], { stdin: input });
  expect(res.exitCode).toBe(0);
  return res.stdout;
}

for (const c of cases) {
  test(c.name, async () => {
    if (c.scenario === "jq-json") {
      const value = z.json().parse(c.value);
      const pretty = z.boolean().parse(c.pretty);
      expect(`${toJqJson(value, pretty)}\n`).toBe(
        await jq(z.array(z.string()).parse(c.jqArgs), JSON.stringify(value)),
      );
      return;
    }
    if (c.scenario === "jq-sort") {
      const words = z.array(z.string()).parse(c.words);
      const sorted = JSON.parse(
        await jq(z.array(z.string()).parse(c.jqArgs), JSON.stringify(words)),
      );
      expect([...words].sort(compareJqStrings)).toEqual(sorted);
      if (c.jsSortDiffers) expect([...words].sort()).not.toEqual(sorted);
      return;
    }
    if (c.scenario === "iso") {
      expect(isoSeconds(new Date(str(c.input)))).toBe(str(c.expected));
      return;
    }
    using sb = createSandbox();
    const file =
      c.scenario === "atomic-missing" ? join(sb.project, str(c.file)) : sb.path(str(c.file));
    if (c.scenario === "atomic-replace") {
      sb.write(str(c.file), str(c.before));
      expect(writeAtomic(file, str(c.after))).toBe(true);
      expect(readFileSync(file, "utf8")).toBe(str(c.after));
      expect(readdirSync(join(sb.project, "state"))).toEqual(
        z.array(z.string()).parse(c.remaining),
      );
    } else if (c.scenario === "atomic-missing") {
      expect(writeAtomic(file, str(c.body))).toBe(false);
      expect(existsSync(file)).toBe(false);
    } else if (c.scenario === "atomic-mode") {
      expect(writeAtomic(file, str(c.body))).toBe(true);
      expect(statSync(file).mode & 0o777).toBe(num(c.mode));
    } else if (c.scenario === "lock-result") {
      expect(withLock(file, () => existsSync(`${file}.lock`))).toBe(true);
      expect(existsSync(`${file}.lock`)).toBe(false);
      expect(() =>
        withLock(file, () => {
          throw new Error(str(c.error));
        }),
      ).toThrow(c.error);
      expect(existsSync(`${file}.lock`)).toBe(false);
    } else if (c.scenario === "dead-pid-lock") {
      const done = await run(["bash", "-c", "echo $$"]);
      writeFileSync(`${file}.lock`, `${done.stdout.trim()}${str(c.lockSuffix)}`);
      const warnings: string[] = [];
      const started = Date.now();
      expect(withLock(file, () => true, { warn: (m) => warnings.push(m) })).toBe(true);
      expect(Date.now() - started).toBeLessThan(num(c.maxMs));
      expect(warnings).toEqual([]);
    } else if (c.scenario === "stale-own-pid") {
      writeFileSync(`${file}.lock`, `${String(process.pid)}${str(c.lockSuffix)}`);
      const old = new Date(Date.now() - num(c.ageMs));
      utimesSync(`${file}.lock`, old, old);
      const warnings: string[] = [];
      expect(withLock(file, () => true, { warn: (m) => warnings.push(m) })).toBe(true);
      expect(warnings).toEqual([]);
      expect(existsSync(`${file}.lock`)).toBe(false);
    } else if (c.scenario === "lock-takeover") {
      withLock(file, () => writeFileSync(`${file}.lock`, str(c.otherLock)));
      expect(readFileSync(`${file}.lock`, "utf8")).toBe(str(c.otherLock));
    } else {
      writeFileSync(`${file}.lock`, str(c.lock));
      if (c.ageMs !== undefined) {
        const old = new Date(Date.now() - c.ageMs);
        utimesSync(`${file}.lock`, old, old);
      }
      if (c.scenario === "stale-no-broken") {
        withLock(file, () => true);
        expect(readdirSync(sb.project).filter((name) => name.includes(".lock"))).toEqual([]);
      } else {
        const warnings: string[] = [];
        const started = Date.now();
        const ran = withLock(file, () => true, {
          ...(c.timeoutMs === undefined ? {} : { timeoutMs: c.timeoutMs }),
          warn: (m) => warnings.push(m),
        });
        expect(ran).toBe(true);
        if (c.scenario === "live-lock") {
          expect(Date.now() - started).toBeGreaterThanOrEqual(num(c.timeoutMs));
          expect(warnings).toEqual([str(materializeCaseValue(sb, c.warning))]);
          expect(existsSync(`${file}.lock`)).toBe(true);
        } else {
          expect(warnings).toEqual(z.array(z.string()).parse(c.expectedWarnings));
          expect(existsSync(`${file}.lock`)).toBe(false);
        }
      }
    }
  });
}
