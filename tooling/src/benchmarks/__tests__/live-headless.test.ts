// Ported from benchmarks/__tests__/live-headless.bats. HERMETIC: no real
// `claude` call and no network. The measurement backend (the usage rollup over
// a real transcript fixture set) and the runner contract are real; the paid
// `claude` CLI is the one substituted boundary — a PATH script that writes the
// real transcript fixture at the pinned session path and prints a real
// --output-format json result, so transcript resolution, rollup, stats and the
// result writer all run for real.
import { expect, test } from "bun:test";
import { chmodSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { validateResult } from "../lib/result.ts";
import { usageRollup } from "../lib/usage.ts";

const REPO = resolve(import.meta.dir, "../../../..");
const FIX = join(REPO, "benchmarks/fixtures");
const WHOLE = join(REPO, "tooling/src/benchmarks/cases/whole-session.ts");

function transcriptSet(): string[] {
  const agents = join(FIX, "sub-session/subagents");
  return [
    join(FIX, "sub-session.jsonl"),
    ...readdirSync(agents)
      .toSorted()
      .map((f) => join(agents, f)),
  ];
}

// sub-session.rollup.json is the bash `stats_usage_rollup` output on this
// fixture set (deleted in #277), minus by_day, which depends on the local zone.
const GOLDEN = z
  .record(z.string(), z.unknown())
  .parse(JSON.parse(readFileSync(join(FIX, "sub-session.rollup.json"), "utf8")));

const GOLDEN_TOTALS = z
  .looseObject({ tokens: z.number(), cache_read: z.number() })
  .parse(GOLDEN["totals"]);

test("the usage rollup reproduces the bash rollup on the real transcript set", () => {
  const { by_day: byDay, ...rest } = usageRollup(transcriptSet());
  const actual: Record<string, unknown> = rest;
  expect(actual).toEqual(GOLDEN);
  const days = Object.values(byDay);
  expect(days.reduce((acc, d) => acc + d.tokens, 0)).toBe(GOLDEN_TOTALS.tokens);
  expect(days.reduce((acc, d) => acc + d.cache_read, 0)).toBe(GOLDEN_TOTALS.cache_read);
});

test("the rollup keeps the final streamed frame per message.id and skips broken lines", () => {
  using sb = createSandbox();
  const [first = ""] = readFileSync(join(FIX, "sub-session.jsonl"), "utf8").split("\n");
  const frame = z
    .looseObject({ message: z.looseObject({ usage: z.looseObject({}) }) })
    .parse(JSON.parse(first));
  const final = {
    ...frame,
    message: { ...frame.message, usage: { ...frame.message.usage, output_tokens: 9999 } },
  };
  const file = sb.write(
    "t.jsonl",
    [first, JSON.stringify(final), first.slice(0, 40), "", "not json"].join("\n"),
  );
  const roll = usageRollup([file]);
  expect(roll.messages).toBe(1);
  expect(roll.totals.output).toBe(9999);
});

test.concurrent("whole-session fails clearly when the claude CLI is absent", async () => {
  const res = await run([process.execPath, WHOLE], { cwd: REPO, env: { PATH: "/usr/bin:/bin" } });
  expect(res.exitCode).not.toBe(0);
  expect(res.stderr).toContain("claude CLI");
});

test.concurrent("whole-session rejects an unknown arg with status 2", async () => {
  expect((await run([process.execPath, WHOLE, "--bogus"], { cwd: REPO })).exitCode).toBe(2);
});

const STUB = `#!/usr/bin/env bun
import { cpSync, mkdirSync } from "node:fs";
const args = process.argv.slice(2);
const sid = args[args.indexOf("--session-id") + 1] ?? "";
const dir = \`\${process.env.CLAUDE_CONFIG_DIR}/projects/\${process.cwd().replace(/[^A-Za-z0-9]/g, "-")}\`;
mkdirSync(\`\${dir}/\${sid}\`, { recursive: true });
cpSync(${JSON.stringify(join(FIX, "sub-session.jsonl"))}, \`\${dir}/\${sid}.jsonl\`);
cpSync(${JSON.stringify(join(FIX, "sub-session/subagents"))}, \`\${dir}/\${sid}/subagents\`, { recursive: true });
process.stdout.write('{"total_cost_usd":0.0123,"result":"ok"}');
`;

const Result = z.looseObject({
  mechanism: z.string(),
  tokenizer: z.looseObject({ mode: z.string() }),
  baseline: z.looseObject({ cost: z.number() }),
  delta: z.looseObject({ tokens_pct: z.number(), cost_pct: z.number() }),
});

test.concurrent("a stubbed claude drives the real emit pipeline (incl. cost) to a schema-valid result", async () => {
  using sb = createSandbox();
  const stub = sb.write("bin/claude", STUB);
  chmodSync(stub, 0o755);
  const bunDir = resolve(process.execPath, "..");
  const res = await run([process.execPath, WHOLE, "--n", "1"], {
    cwd: REPO,
    env: {
      PATH: `${sb.path("bin")}:${bunDir}:/usr/bin:/bin`,
      CLAUDE_CONFIG_DIR: sb.path("cfg"),
      BENCH_RESULTS_DIR: sb.path("results"),
    },
    timeoutMs: 60_000,
  });
  expect(res.exitCode).toBe(0);
  const out = res.stdout.trim();
  expect(out.startsWith(sb.path("results/whole-session-live-"))).toBe(true);
  const doc = Result.parse(JSON.parse(readFileSync(out, "utf8")));
  expect([doc.mechanism, doc.tokenizer.mode]).toEqual(["whole-session", "usage"]);
  expect(typeof doc.delta.tokens_pct).toBe("number");
  expect(doc.baseline.cost).toBeGreaterThan(0);
  expect(typeof doc.delta.cost_pct).toBe("number");
  expect(() => validateResult(out)).not.toThrow();
});

test("a hand-built whole-session result passes validation", () => {
  using sb = createSandbox();
  const file = sb.write("whole-session-live-2026-06-15.json", {
    mechanism: "whole-session",
    tier: "live",
    method: "headless-ab",
    tokenizer: { mode: "usage", source: "usage" },
    provenance: {
      model: "claude-sonnet-4-6",
      date: "2026-06-15",
      commit: "deadbeef",
      n_runs: 5,
      pricing_id: "2026-06",
    },
    baseline: {
      label: "toolu-off",
      tokens: { input: null, output: null, cache_read: null, cache_write: null, total: 120000 },
      cost: 0.42,
    },
    treatment: {
      label: "toolu-on",
      tokens: { input: null, output: null, cache_read: null, cache_write: null, total: 95000 },
      cost: 0.31,
    },
    delta: { tokens_pct: 20, cost_pct: 26, abs_tokens: 25000, mean: 0.31, stddev: 0.02 },
    cases: [{ id: "summarize-bench-lib", baseline_tokens: 120000, treatment_tokens: 95000 }],
    notes: "AGGREGATE whole-session delta; NOT the sum of per-mechanism deltas.",
  });
  expect(() => validateResult(file)).not.toThrow();
});

test("an unreadable transcript is skipped without disturbing the rest of the rollup", () => {
  using sb = createSandbox();
  expect(usageRollup([sb.path("missing.jsonl"), ...transcriptSet()])).toEqual(
    usageRollup(transcriptSet()),
  );
});
