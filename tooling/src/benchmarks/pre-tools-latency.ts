/**
 * PreToolUse hook latency (#258, #260; epic #247 budget: p50 no worse than the
 * bash baseline + 5 ms on the same machine), spawned the way Claude Code
 * spawns hooks:
 *
 *   bash     `bash pre-tools/mod.sh` and `bash mcp-blocker.sh` from
 *            `plugins/toolu` at 2386d4f3 (`git archive`), the last commit where
 *            every module ran on bash
 *   bundle   the committed `hooks/dist/pre-tools.js` and `hooks/dist/mcp-tools.js`
 *            behind their launchers: protected-files, mcp-blocker and
 *            code-edit-rules native (#260), the rest on their bash fallback
 *
 * Each sample pair runs bash then the bundle back to back, so load drift on a
 * shared machine hits both sides alike.
 *
 * Usage: bun run tooling/src/benchmarks/pre-tools-latency.ts [--runs N] [--assert]
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mcpFixture, toStdin } from "@toolu/conformance/harness/fixtures";
import { launcherCommand } from "@toolu/core/launcher";
import {
  pretoolEnv,
  REPO_ROOT,
  runBundle,
  type PretoolRun,
} from "@toolu/conformance/harness/pretool";
import { PRETOOL_CORPUS, prepare } from "@toolu/conformance/harness/pretool-corpus";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { percentile } from "@toolu/conformance/harness/timing";

const BUDGET_MS = 5;
const SLICE = [
  "protected-files: .env edit asks",
  "code-edit-rules: feature docs",
  "multi-file patch: advisories only",
  "plain command is silent",
  "commit-gate: commit advice",
  "push-review + docs-sync: unreviewed push advises",
  "ast-grep registry: structural Grep nudge",
];

/** The last commit where protected-files, mcp-blocker and code-edit-rules ran on bash. */
const BASE = "2386d4f3";

const MCP_CASES = [
  { name: "mcp: listed server asks", list: "exampleblocked\n", server: "exampleblocked" },
  { name: "mcp: unlisted server", list: "exampleblocked\n", server: "other" },
  { name: "mcp: no blocklist, no config", list: undefined, server: "other" },
] as const;

/** `plugins/toolu` at `BASE`, extracted into `dir`. */
function baseTree(dir: string): string {
  const known = spawnSync("git", ["cat-file", "-e", `${BASE}^{commit}`], { cwd: REPO_ROOT });
  if (known.status !== 0) {
    throw new Error(
      `commit ${BASE} is not in this clone (shallow?); fetch it to measure the bash baseline`,
    );
  }
  const extract = spawnSync(
    "sh",
    ["-c", 'git archive "$1" plugins/toolu | tar -x -C "$2"', "sh", BASE, dir],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  if (extract.status !== 0) throw new Error(`extracting ${BASE} failed: ${extract.stderr}`);
  return join(dir, "plugins/toolu/hooks");
}

type Row = { name: string; bash: number; candidate: number };

type Side = () => Promise<RunResult>;

/** Run `step` for each item strictly one after another: concurrent spawns would skew each other. */
function inSequence<T, R>(items: readonly T[], step: (item: T) => Promise<R>): Promise<R[]> {
  return items.reduce<Promise<R[]>>(
    async (done, item) => [...(await done), await step(item)],
    Promise.resolve([]),
  );
}

/** p50 of each side over `runs` back-to-back pairs, after two warmup pairs. */
async function paired(bash: Side, bundle: Side, runs: number): Promise<[number, number]> {
  const pairs = await inSequence([...Array.from({ length: runs + 2 }).keys()], async () => {
    const a = await bash();
    const b = await bundle();
    return [a.durationMs, b.durationMs] as const;
  });
  const measured = pairs.slice(2);
  return [
    percentile(
      measured.map((p) => p[0]),
      50,
    ),
    percentile(
      measured.map((p) => p[1]),
      50,
    ),
  ];
}

async function measureCorpus(hooks: string, name: string, runs: number): Promise<Row> {
  const c = PRETOOL_CORPUS.find((entry) => entry.name === name);
  if (c === undefined) throw new Error(`no corpus case named ${name}`);
  using sb = createSandbox({ git: true });
  const extra = await prepare(sb, "claude", c);
  const stdin = c.stdin ?? JSON.stringify(toStdin("claude", c.fixture(sb), { cwd: sb.project }));
  const call: PretoolRun = { cwd: sb.project, env: pretoolEnv(sb, "claude", extra), stdin };
  const modSh = join(hooks, "pre-tools/mod.sh");
  const [bash, bundle] = await paired(
    () => run(["bash", modSh], call),
    () => runBundle(call),
    runs,
  );
  return { name, bash, candidate: bundle };
}

async function measureMcp(
  hooks: string,
  c: (typeof MCP_CASES)[number],
  runs: number,
): Promise<Row> {
  using sb = createSandbox({ git: true });
  const settings = join(sb.root, "settings");
  mkdirSync(settings);
  if (c.list !== undefined) writeFileSync(join(settings, "mcp-blocklist.txt"), c.list);
  const fixture = mcpFixture(c.server, "search", {});
  const stdin = JSON.stringify(toStdin("claude", fixture, { cwd: sb.project }));
  const call = {
    cwd: sb.project,
    env: pretoolEnv(sb, "claude", { TOOLU_SETTINGS_DIR: settings }),
    stdin,
  };
  const command = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: "mcp-tools" });
  const script = join(hooks, "pre-tools/modules/mcp-blocker.sh");
  const [bash, bundle] = await paired(
    () => run(["bash", script], call),
    () => run(["/bin/sh", "-c", command], call),
    runs,
  );
  return { name: c.name, bash, candidate: bundle };
}

function ms(value: number): string {
  return value.toFixed(1);
}

function report(rows: readonly Row[]): string {
  const lines = [
    "| Fixture | bash p50 | bundle p50 | bundle − bash |",
    "|---|---|---|---|",
    ...rows.map(
      (r) => `| ${r.name} | ${ms(r.bash)} | ${ms(r.candidate)} | ${ms(r.candidate - r.bash)} |`,
    ),
  ];
  return `${lines.join("\n")}\n`;
}

function overBudget(rows: readonly Row[]): string[] {
  return rows
    .filter((r) => r.candidate > r.bash + BUDGET_MS)
    .map(
      (r) =>
        `${r.name}: bundle p50 ${ms(r.candidate)} ms > bash ${ms(r.bash)} ms + ${String(BUDGET_MS)} ms`,
    );
}

async function main(argv: readonly string[]): Promise<number> {
  const runsAt = argv.indexOf("--runs");
  const runs = runsAt === -1 ? 15 : Number(argv[runsAt + 1]);
  const dir = mkdtempSync(join(tmpdir(), "toolu-pretool-bench-"));
  try {
    const hooks = baseTree(dir);
    const rows = [
      ...(await inSequence(SLICE, (name) => measureCorpus(hooks, name, runs))),
      ...(await inSequence(MCP_CASES, (c) => measureMcp(hooks, c, runs))),
    ];
    process.stdout.write(report(rows));
    const over = overBudget(rows);
    for (const line of over) process.stderr.write(`pre-tools-latency: ${line}\n`);
    return argv.includes("--assert") && over.length > 0 ? 1 : 0;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

process.exitCode = await main(process.argv.slice(2));
