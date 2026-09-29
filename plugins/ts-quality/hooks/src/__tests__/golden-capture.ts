/**
 * One-time capture of the bash ts-quality module (#265): `git archive` the
 * plugin's hooks at `--base` (default a8b0c9c9, the last commit with bash),
 * register that module in each case's sandbox and record every step through
 * the post-tools bundle into `fixtures/golden.json`. Reading git objects keeps
 * it reproducible after the bash files are deleted.
 *
 * Usage: bun run plugins/ts-quality/hooks/src/__tests__/golden-capture.ts [--base <sha>]
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { TS_CASES } from "./cases.ts";
import { GOLDEN_PATH, REPO_ROOT, caseKey, runCase, type StepResult } from "./golden-harness.ts";

const DEFAULT_BASE = "a8b0c9c9";

function git(args: string[]): string {
  const res = spawnSync("git", ["-C", REPO_ROOT, ...args], { encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
  return res.stdout.trim();
}

/** The base commit's plugins/ts-quality/hooks, extracted into `dir`. */
function extract(base: string, dir: string): string {
  const tar = spawnSync("git", ["-C", REPO_ROOT, "archive", base, "plugins/ts-quality/hooks"]);
  if (tar.status !== 0) throw new Error(`git archive ${base} failed`);
  const untar = spawnSync("tar", ["-x", "-C", dir], { input: tar.stdout });
  if (untar.status !== 0) throw new Error("tar -x failed");
  return join(dir, "plugins/ts-quality/hooks/register.sh");
}

const at = process.argv.indexOf("--base");
const base = git([
  "rev-parse",
  `${at === -1 ? DEFAULT_BASE : (process.argv[at + 1] ?? "")}^{commit}`,
]);
const dir = mkdtempSync(join(tmpdir(), "ts-quality-base-"));
try {
  const register = extract(base, dir);
  const jobs = TS_CASES.flatMap((c) =>
    (c.hosts ?? ["claude"]).map(async (host) => {
      const steps = await runCase(c, host, { kind: "bash", register });
      return [caseKey(c, host), steps] as [string, StepResult[]];
    }),
  );
  const cases = Object.fromEntries(await Promise.all(jobs));
  mkdirSync(dirname(GOLDEN_PATH), { recursive: true });
  writeFileSync(GOLDEN_PATH, `${JSON.stringify({ base, cases }, null, 2)}\n`);
  process.stdout.write(`captured ${String(Object.keys(cases).length)} cases at ${base}\n`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
