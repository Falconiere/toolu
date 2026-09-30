/**
 * One-time capture of the bash rust-quality module (#267): `git archive` the
 * plugin's hooks at `--base` (default c50c6bd9, the last commit with bash),
 * register that module in each case's sandbox and record every step through
 * the post-tools bundle into `fixtures/golden.json`. Reading git objects keeps
 * it reproducible after the bash files are deleted.
 *
 * Usage: bun run plugins/rust-quality/hooks/src/__tests__/golden-capture.ts [--base <sha>]
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { RS_CASES } from "./cases.ts";
import {
  BASH_BASE,
  GOLDEN_PATH,
  REPO_ROOT,
  caseKey,
  extractBaseRegister,
  runCase,
  type StepResult,
} from "./golden-harness.ts";

/** Cases run this many at a time: all at once starves each spawn past the harness timeout. */
const BATCH = 8;

async function inBatches<T>(jobs: readonly (() => Promise<T>)[], size: number): Promise<T[]> {
  const out: T[] = [];
  for (let at = 0; at < jobs.length; at += size) {
    out.push(...(await Promise.all(jobs.slice(at, at + size).map((job) => job()))));
  }
  return out;
}

function git(args: string[]): string {
  const res = spawnSync("git", ["-C", REPO_ROOT, ...args], { encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
  return res.stdout.trim();
}

const SUITES = "plugins/rust-quality/hooks/concerns/__tests__";

/** Every `@test` at `base` in the concern suites, as `<suite>.bats: <title>`. */
function batsTests(base: string): string[] {
  return git(["ls-tree", "--name-only", `${base}:${SUITES}`])
    .split("\n")
    .filter((name) => name.endsWith(".bats"))
    .flatMap((name) =>
      [
        ...git(["show", `${base}:${SUITES}/${name}`]).matchAll(/^@test "((?:[^"\\]|\\.)*)" \{$/gm),
      ].map((m) => `${name}: ${(m[1] ?? "").replaceAll('\\"', '"')}`),
    );
}

const at = process.argv.indexOf("--base");
const baseArg = at === -1 ? BASH_BASE : process.argv[at + 1];
if (baseArg === undefined || baseArg.trim() === "") {
  process.stderr.write("--base requires a non-empty commit\n");
  process.stderr.write(
    "Usage: bun run plugins/rust-quality/hooks/src/__tests__/golden-capture.ts [--base <sha>]\n",
  );
  process.exit(2);
}
const base = git(["rev-parse", `${baseArg}^{commit}`]);
const dir = mkdtempSync(join(tmpdir(), "rust-quality-base-"));
try {
  const register = extractBaseRegister(base, dir);
  const jobs = RS_CASES.flatMap((c) =>
    (c.hosts ?? ["claude"]).map((host) => async (): Promise<[string, StepResult[]]> => [
      caseKey(c, host),
      await runCase(c, host, { kind: "bash", register }),
    ]),
  );
  const cases = Object.fromEntries(await inBatches(jobs, BATCH));
  mkdirSync(dirname(GOLDEN_PATH), { recursive: true });
  const golden = { base, batsTests: batsTests(base), cases };
  writeFileSync(GOLDEN_PATH, `${JSON.stringify(golden, null, 2)}\n`);
  process.stdout.write(`captured ${String(Object.keys(cases).length)} cases at ${base}\n`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
