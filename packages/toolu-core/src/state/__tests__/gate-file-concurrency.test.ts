/** Real concurrent writers and reader polling over shared state fixture cases. */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readdirSync, utimesSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { readGateFile } from "../gate-file.ts";

const WRITER = resolve(import.meta.dir, "gate-writer.ts");
const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("concurrency"),
  scenario: z.enum(["race", "stale-lock"]),
  branch: z.string(),
  writers: z.number().int().optional(),
  iterations: z.number().int(),
  mode: z.string(),
  keptIndices: z.array(z.number().int()).optional(),
  minReads: z.number().int().optional(),
  timeoutMs: z.number().int().optional(),
  lock: z.string().optional(),
  ageMs: z.number().int().optional(),
  writerId: z.string().optional(),
  maxMs: z.number().int().optional(),
  expectedEntries: z.array(z.string()).optional(),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/state/cases.json"))
  .filter((raw) => raw.kind === "concurrency")
  .map((raw) => CaseSchema.parse(raw));

function setup(branch: string): { sb: Sandbox; gate: string; env: Record<string, string> } {
  const sb = createSandbox({ git: true, branch });
  sb.writeConfig("claude", "project", { version: 1 });
  const dir = join(sb.project, ".claude", "tmp");
  mkdirSync(dir, { recursive: true });
  const env = { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: "claude" };
  return { sb, gate: join(dir, "quality-gate-status.json"), env };
}

async function watch(gate: string, done: Promise<unknown>, minReads: number): Promise<string[]> {
  const bad: string[] = [];
  let finished = false;
  void done.finally(() => {
    finished = true;
  });
  let reads = 0;
  while (!finished) {
    const read = readGateFile(gate);
    if (read.kind !== "ok" && read.kind !== "missing") bad.push(`${read.kind}: ${read.reason}`);
    reads += 1;
    await Bun.sleep(1);
  }
  expect(reads).toBeGreaterThan(minReads);
  return bad;
}

function expectAllOk(results: RunResult[]): void {
  for (const res of results)
    expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
}

for (const c of cases) {
  test(
    c.name,
    async () => {
      const { sb, gate, env } = setup(c.branch);
      using _sb = sb;
      if (c.scenario === "race") {
        const count = z.number().int().parse(c.writers);
        const writers = Array.from({ length: count }, (_, id) =>
          run(["bun", WRITER, gate, String(id), String(c.iterations), c.mode], {
            cwd: sb.project,
            env,
            timeoutMs: z.number().int().parse(c.timeoutMs),
          }),
        );
        const all = Promise.all(writers);
        const bad = await watch(gate, all, z.number().int().parse(c.minReads));
        expectAllOk(await all);
        expect(bad).toEqual([]);
        const read = readGateFile(gate);
        if (read.kind !== "ok" || read.doc.status !== "failing")
          throw new Error(`unexpected ${read.kind}`);
        const kept = z.array(z.number().int()).parse(c.keptIndices);
        const expected = Array.from({ length: count }, (_, id) =>
          kept.map((index) => `/w/${String(id)}/${String(index)}`),
        ).flat();
        expect(Object.keys(read.doc.entries ?? {}).sort()).toEqual(expected.sort());
        expect(read.doc.violations.split("\n").filter(Boolean).sort()).toEqual(
          expected.map((file) => file.slice(3)).sort(),
        );
        expect(
          readdirSync(join(sb.project, ".claude", "tmp")).filter((n) => /\.(tmp|lock)$/.test(n)),
        ).toEqual([]);
      } else {
        writeFileSync(`${gate}.lock`, z.string().parse(c.lock));
        const old = new Date(Date.now() - z.number().int().parse(c.ageMs));
        utimesSync(`${gate}.lock`, old, old);
        const started = Date.now();
        expectAllOk([
          await run(
            ["bun", WRITER, gate, z.string().parse(c.writerId), String(c.iterations), c.mode],
            { cwd: sb.project, env },
          ),
        ]);
        expect(Date.now() - started).toBeLessThan(z.number().int().parse(c.maxMs));
        expect(existsSync(`${gate}.lock`)).toBe(false);
        const read = readGateFile(gate);
        expect<unknown>(
          read.kind === "ok" &&
            read.doc.status === "failing" &&
            Object.keys(read.doc.entries ?? {}),
        ).toEqual(z.array(z.string()).parse(c.expectedEntries));
      }
    },
    120_000,
  );
}
