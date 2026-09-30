/**
 * One-time capture of the bash ast-grep plugin (#268): `git archive` it at
 * `--base` (default 2912cd9d, the last commit with bash), register its modules
 * with that `register.sh` in each case's sandbox, and record every case through
 * toolu's dispatcher bundles, plus the bash report, into `fixtures/golden.json`.
 * Reading git objects keeps it reproducible after the bash files are deleted.
 *
 * Usage: bun run plugins/ast-grep/hooks/src/__tests__/golden-capture.ts [--base <sha>]
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { NUDGE_CASES } from "./cases-nudge.ts";
import { REPORT_CASES } from "./cases-report.ts";
import { SAVINGS_CASES } from "./cases-savings.ts";
import {
  CORPUS,
  caseKey,
  hostsOf,
  runCorpus,
  runNudge,
  runReport,
  runSavings,
} from "./golden-harness.ts";
import {
  BASH_BASE,
  GOLDEN_PATH,
  REPO_ROOT,
  extractBase,
  type Registration,
} from "./golden-sandbox.ts";

/** Cases run this many at a time: all at once starves each spawn past the harness timeout. */
const BATCH = 8;

async function inBatches<T>(
  jobs: readonly (() => Promise<[string, T]>)[],
): Promise<Record<string, T>> {
  const out: [string, T][] = [];
  for (let at = 0; at < jobs.length; at += BATCH) {
    out.push(...(await Promise.all(jobs.slice(at, at + BATCH).map((job) => job()))));
  }
  return Object.fromEntries(out);
}

const HOSTS = ["claude", "codex"] as const;

async function keyed<T>(key: string, result: Promise<T>): Promise<[string, T]> {
  return [key, await result];
}

function revParse(rev: string): string {
  const res = spawnSync("git", ["-C", REPO_ROOT, "rev-parse", `${rev}^{commit}`], {
    encoding: "utf8",
  });
  if (res.status !== 0) throw new Error(`git rev-parse ${rev}: ${res.stderr}`);
  return res.stdout.trim();
}

const at = process.argv.indexOf("--base");
const base = revParse(at === -1 ? BASH_BASE : (process.argv[at + 1] ?? ""));
const dir = mkdtempSync(join(tmpdir(), "ast-grep-base-"));
try {
  const reg: Registration = { kind: "bash", pluginRoot: extractBase(base, dir) };
  const nudge = await inBatches(
    NUDGE_CASES.flatMap((c) =>
      hostsOf(c).map((host) => () => keyed(caseKey(c.name, host), runNudge(c, host, reg))),
    ),
  );
  const savings = await inBatches(
    SAVINGS_CASES.flatMap((c) =>
      hostsOf(c).map((host) => () => keyed(caseKey(c.name, host), runSavings(c, host, reg))),
    ),
  );
  const report = await inBatches(REPORT_CASES.map((c) => () => keyed(c.name, runReport(c, reg))));
  const corpus = await inBatches(
    CORPUS.flatMap((f) =>
      HOSTS.map((host) => () => keyed(caseKey(f.name, host), runCorpus(f, host, reg))),
    ),
  );
  mkdirSync(dirname(GOLDEN_PATH), { recursive: true });
  writeFileSync(
    GOLDEN_PATH,
    `${JSON.stringify({ base, nudge, savings, report, corpus }, null, 2)}\n`,
  );
  const counts = [nudge, savings, report, corpus].map((m) => Object.keys(m).length).join("/");
  process.stdout.write(`captured nudge/savings/report/corpus ${counts} at ${base}\n`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
