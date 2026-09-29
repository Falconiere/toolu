// Ported from benchmarks/__tests__/{retrieval,run}.bats: the deterministic
// retrieval bench over the stable fixture corpus, and the dispatcher's
// contract. Hermetic: heuristic counting, results in a temp dir, real ast-grep.
import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

const REPO = resolve(import.meta.dir, "../../../..");
const FIX = join(REPO, "benchmarks/fixtures");
const RETRIEVAL = join(REPO, "tooling/src/benchmarks/cases/retrieval.ts");
const RUN = join(REPO, "tooling/src/benchmarks/run.ts");

const Result = z.looseObject({
  tokenizer: z.looseObject({ mode: z.string() }),
  delta: z.looseObject({ tokens_pct: z.number() }),
  notes: z.string(),
  cases: z.array(z.looseObject({ id: z.string(), saved: z.number(), note: z.string().optional() })),
});

async function bench(args: string[], results: string): ReturnType<typeof run> {
  return run([process.execPath, ...args], {
    cwd: REPO,
    env: { BENCH_RESULTS_DIR: results, ANTHROPIC_API_KEY: "" },
    timeoutMs: 60_000,
  });
}

test.concurrent("retrieval over the fixture corpus: schema-valid, heuristic, positive savings, guarded missing target", async () => {
  using sb = createSandbox();
  const res = await bench(
    [RETRIEVAL, "--queries", join(FIX, "queries.tsv"), "--corpus", FIX],
    sb.path("results"),
  );
  expect(res.exitCode).toBe(0);
  const out = res.stdout.trim();
  expect((await bench([RUN, "--validate", out], sb.path("results"))).exitCode).toBe(0);
  const doc = Result.parse(JSON.parse(readFileSync(out, "utf8")));
  expect(doc.tokenizer.mode).toBe("heuristic");
  expect(doc.cases.find((c) => c.id === "target")?.saved).toBeGreaterThan(0);
  expect(doc.delta.tokens_pct).toBeGreaterThan(0);
  expect(doc.cases.find((c) => c.id === "missing")).toMatchObject({
    saved: 0,
    note: "missing-or-empty-target",
  });
  expect(doc.notes).toContain("missing-target");
});

test.concurrent("--tier deterministic runs retrieval and writes a result", async () => {
  using sb = createSandbox();
  const res = await bench([RUN, "--tier", "deterministic"], sb.path("results"));
  expect(res.exitCode).toBe(0);
  expect(
    readdirSync(sb.path("results")).filter((f) => f.startsWith("retrieval-deterministic-")),
  ).toHaveLength(1);
});

test.concurrent("bad arguments exit 2; --help exits 0 with usage", async () => {
  using sb = createSandbox();
  expect((await bench([RUN, "--bogus"], sb.path("r"))).exitCode).toBe(2);
  expect((await bench([RUN, "--tier", "nonsense"], sb.path("r"))).exitCode).toBe(2);
  expect((await bench([RETRIEVAL, "--bogus", "x"], sb.path("r"))).exitCode).toBe(2);
  const help = await bench([RUN, "--help"], sb.path("r"));
  expect(help.exitCode).toBe(0);
  expect(help.stdout).toContain("usage:");
});

test.concurrent("--validate passes a good result and fails a bad one", async () => {
  using sb = createSandbox();
  await bench([RUN, "--tier", "deterministic"], sb.path("results"));
  const [good = ""] = readdirSync(sb.path("results"));
  expect(
    (await bench([RUN, "--validate", sb.path(`results/${good}`)], sb.path("results"))).exitCode,
  ).toBe(0);
  const bad = sb.write("bad.json", '{"mechanism":"retrieval"}\n');
  expect((await bench([RUN, "--validate", bad], sb.path("results"))).exitCode).not.toBe(0);
});
