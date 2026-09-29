// Ported from benchmarks/__tests__/{common,tokens,result,docs}.bats: the
// harness bootstrap, token counting, run stats, the result writer/validator
// and the methodology contract. Real files and a real (unroutable) endpoint.
import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { PRICING_ID, rates } from "../lib/pricing.ts";
import { ResultError, validateResult, writeResult } from "../lib/result.ts";
import { benchRoot, resultsDir } from "../lib/root.ts";
import { TokenCountError, countTokens, stats } from "../lib/tokens.ts";
import { usageRollup } from "../lib/usage.ts";

const REPO = resolve(import.meta.dir, "../../../..");

function fixtureResult(modes: string[]): Record<string, unknown> {
  return {
    mechanism: "retrieval",
    tier: "deterministic",
    method: "tool-bytes",
    tokenizer: { mode: "heuristic", source: "bytes-div-4" },
    provenance: {
      model: null,
      date: "2026-06-14",
      commit: "abc123",
      n_runs: 1,
      pricing_id: "2026-06",
    },
    baseline: {
      label: "full-read",
      tokens: { input: 400, output: 0, cache_read: 0, cache_write: 0, total: 400 },
      cost: null,
    },
    treatment: {
      label: "ast-grep",
      tokens: { input: 80, output: 0, cache_read: 0, cache_write: 0, total: 80 },
      cost: null,
    },
    delta: { tokens_pct: 80, cost_pct: null, abs_tokens: 320, mean: null, stddev: null },
    cases: [],
    notes: "",
    _modes: modes,
  };
}

test("the bench root is the git toplevel; results default under benchmarks/results", () => {
  const top = spawnSync("git", ["-C", import.meta.dir, "rev-parse", "--show-toplevel"], {
    encoding: "utf8",
  }).stdout.trim();
  expect(benchRoot()).toBe(top);
  expect(resultsDir(top, {})).toBe(join(top, "benchmarks/results"));
  expect(resultsDir(top, { BENCH_RESULTS_DIR: "/tmp/x" })).toBe("/tmp/x");
});

test("the token math is benchmarks' own: pricing id, rates, rollup exported", () => {
  expect(PRICING_ID).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  // Family ordering and the flagged fallback, not today's exact prices.
  expect(rates("claude-haiku-4-5").i).toBeLessThan(rates("claude-sonnet-4-6").i);
  expect(rates("claude-sonnet-4-6").i).toBeLessThan(rates("claude-opus-4-8").i);
  expect(rates("claude-sonnet-4-6").unknown).toBeUndefined();
  expect(rates("mystery-model")).toEqual({ ...rates("claude-sonnet-4-6"), unknown: true });
  expect(typeof usageRollup).toBe("function");
});

test("heuristic mode: bytes/4 when no API key", async () => {
  expect(await countTokens("abcdefgh", { apiKey: "" })).toEqual({
    tokens: 2,
    mode: "heuristic",
    source: "bytes-div-4",
  });
});

test("no silent downgrade: a set key with an unreachable API aborts", async () => {
  const err: unknown = await countTokens("hello", {
    apiKey: "test-key",
    baseUrl: "http://127.0.0.1:9",
  }).then(
    () => null,
    (reason: unknown) => reason,
  );
  expect(err).toBeInstanceOf(TokenCountError);
  expect(err instanceof Error ? err.message : "").toContain("refusing to downgrade");
});

test.skipIf((process.env["ANTHROPIC_API_KEY"] ?? "") === "")(
  "exact mode matches API input_tokens (key-gated)",
  async () => {
    const res = await countTokens("The quick brown fox.", {
      apiKey: process.env["ANTHROPIC_API_KEY"],
    });
    expect(res.mode).toBe("exact");
    expect(res.tokens).toBeGreaterThan(0);
  },
);

test("stats: mean/stddev/n over real samples; one sample has zero stddev", () => {
  const three = stats([10, 20, 30]);
  expect([three.mean, three.n, Math.round(three.stddev)]).toEqual([20, 3, 10]);
  expect(stats([42])).toEqual({ mean: 42, stddev: 0, n: 1 });
  expect(stats([])).toEqual({ mean: 0, stddev: 0, n: 0 });
});

test("writes a valid result, returns its path, strips _modes", () => {
  using sb = createSandbox();
  const out = writeResult(fixtureResult(["heuristic", "heuristic"]), sb.path("results"));
  expect(out).toBe(sb.path("results/retrieval-deterministic-2026-06-14.json"));
  expect(JSON.parse(readFileSync(out, "utf8"))).not.toHaveProperty("_modes");
  expect(() => validateResult(out)).not.toThrow();
});

test("mixed tokenizer modes are refused", () => {
  using sb = createSandbox();
  expect(() => writeResult(fixtureResult(["exact", "heuristic"]), sb.path("results"))).toThrow(
    "mixed tokenizer modes",
  );
  expect(existsSync(sb.path("results"))).toBe(false);
});

test("validate names a missing required key, and a missing file", () => {
  using sb = createSandbox();
  const bad = sb.write("bad.json", {
    mechanism: "retrieval",
    tier: "deterministic",
    method: "tool-bytes",
    tokenizer: { mode: "heuristic", source: "bytes-div-4" },
    provenance: {
      model: null,
      date: "2026-06-14",
      commit: "abc",
      n_runs: 1,
      pricing_id: "2026-06",
    },
    baseline: { label: "full-read", tokens: { total: 400 } },
    treatment: { label: "ast-grep", tokens: { total: 80 } },
    delta: { abs_tokens: 320 },
  });
  expect(() => validateResult(bad)).toThrow("delta.tokens_pct");
  expect(() => validateResult(sb.path("nope.json"))).toThrow(ResultError);
});

test("the methodology contract states the load-bearing rules", () => {
  expect(existsSync(join(REPO, "benchmarks/README.md"))).toBe(true);
  const methodology = readFileSync(
    join(REPO, "benchmarks/results/README.md"),
    "utf8",
  ).toLowerCase();
  for (const rule of [
    "methodology",
    "fair baseline",
    "pins its baseline",
    "never mixed",
    "message.usage",
    "provenance",
  ]) {
    expect(methodology).toContain(rule);
  }
});
