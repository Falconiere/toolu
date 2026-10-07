/**
 * The shared ledger golden (#421), `fixtures/ledger/cases.json`: each case is a
 * sequence of steps over one sandboxed repository (`main` with `base.txt`,
 * then `feat/x` adding `a.ts`). A step writes a file, runs a shell script, or
 * runs `plan-ledger` or `verdict` through the `TOOLU_IMPL` seam and compares
 * its streams, exit code and the branch ledger with the recorded ones. The
 * Rust binary replays the same file (`crates/cli/tests/ledger_cases.rs`).
 */
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run as spawn } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

export const FIXTURE = resolve(import.meta.dir, "../../../../../fixtures/ledger/cases.json");
export const LEDGER = ".claude/tmp/plan-ledger/feat_x.json";

const ExpectSchema = z.strictObject({
  exitCode: z.number().int(),
  stdout: z.string(),
  stderr: z.string(),
  ledger: z.string().nullable().optional(),
});
const RunSchema = z.strictObject({
  op: z.literal("run"),
  cli: z.enum(["plan-ledger", "verdict"]),
  argv: z.array(z.string()),
  cwd: z.string().optional(),
  env: z.record(z.string(), z.string()).optional(),
  expect: ExpectSchema,
});
const StepSchema = z.discriminatedUnion("op", [
  z.strictObject({ op: z.literal("write"), path: z.string().min(1), body: z.string() }),
  z.strictObject({ op: z.literal("sh"), script: z.string().min(1) }),
  RunSchema,
]);
export const CaseSchema = z.strictObject({
  name: z.string().min(1),
  env: z.record(z.string(), z.string()).optional(),
  steps: z.array(StepSchema).min(1),
});
export type LedgerCase = z.infer<typeof CaseSchema>;
export type RunStep = z.infer<typeof RunSchema>;
export type Observed = z.infer<typeof ExpectSchema>;

const BASELINE =
  "git init -q -b main && echo base > base.txt && git add . && git commit -qm base " +
  "&& git checkout -q -b feat/x && echo x > a.ts && git add a.ts && git commit -qm a";

/** The case's sandbox paths, longest first, and the tokens that stand for them. */
export type Paths = { project: string; home: string };

/** `$PROJECT` and `$HOME` in a case string. */
export function expand(text: string, paths: Paths): string {
  return text.replaceAll("$PROJECT", paths.project).replaceAll("$HOME", paths.home);
}

/**
 * Output as the fixture records it: sandbox paths as tokens, and the parts
 * that change from run to run (timestamps, durations, hashes) as placeholders.
 */
export function normalize(text: string, paths: Paths): string {
  return text
    .replaceAll(paths.project, "$PROJECT")
    .replaceAll(paths.home, "$HOME")
    .replace(/\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d{3})?Z/g, "<TS>")
    .replace(/\(\d+s\)/g, "(<N>s)")
    .replace(/\b[0-9a-f]{40}\b/g, "<SHA>")
    .replace(/\(diff [0-9a-f]{12}\)/g, "(diff <SHA>)");
}

/** Variables of the developer's or CI's shell that would change a ledger or verdict answer. */
const UNSET = [
  "STATE_DIR",
  "LEDGER_DIR",
  "DOCS_SYNC_BASE",
  "DOCS_SYNC_STATE_DIR",
  "TELEMETRY_DIR",
  "PL_STUCK_THRESHOLD",
  "PLAN_LEDGER_STEP_TIMEOUT",
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
];

function environment(c: LedgerCase, extra: Record<string, string>, paths: Paths) {
  const env: Record<string, string | undefined> = Object.fromEntries(
    UNSET.map((key) => [key, undefined]),
  );
  Object.assign(env, {
    PATH: process.env.PATH ?? "",
    HOME: paths.home,
    TOOLU_CONFIG_DIR: paths.home,
    TOOLU_HOST_OVERRIDE: "claude",
    PUSH_REVIEW_BASE: "main",
    GIT_AUTHOR_NAME: "t",
    GIT_AUTHOR_EMAIL: "t@t",
    GIT_COMMITTER_NAME: "t",
    GIT_COMMITTER_EMAIL: "t@t",
    GIT_CONFIG_NOSYSTEM: "1",
  });
  for (const [key, value] of Object.entries({ ...c.env, ...extra }))
    env[key] = expand(value, paths);
  return env;
}

async function sh(
  script: string,
  cwd: string,
  env: Record<string, string | undefined>,
): Promise<void> {
  const result = await spawn(["bash", "-c", script], { cwd, env });
  if (result.exitCode !== 0) throw new Error(`${script}: ${result.stderr}`);
}

/** One CLI run, observed and normalized. */
async function observe(c: LedgerCase, step: RunStep, paths: Paths): Promise<Observed> {
  const argv = [...entryArgv("toolu", step.cli), ...step.argv.map((word) => expand(word, paths))];
  const cwd = step.cwd === undefined ? paths.project : expand(step.cwd, paths);
  const result = await spawn(argv, { cwd, env: environment(c, step.env ?? {}, paths) });
  let ledger: string | null = null;
  try {
    ledger = normalize(readFileSync(join(paths.project, LEDGER), "utf8"), paths);
  } catch {
    ledger = null;
  }
  return {
    exitCode: result.exitCode ?? -1,
    stdout: normalize(result.stdout, paths),
    stderr: normalize(result.stderr, paths),
    ledger,
  };
}

/** Run every step of `c`; `check` receives each run with what it produced. */
export async function runCase(
  c: LedgerCase,
  check: (step: RunStep, observed: Observed, index: number) => void,
): Promise<void> {
  using sb: Sandbox = createSandbox();
  const root = realpathSync(sb.root);
  const paths = { project: join(root, "repo"), home: join(root, "user") };
  mkdirSync(paths.project, { recursive: true });
  mkdirSync(paths.home, { recursive: true });
  await sh(BASELINE, paths.project, environment(c, {}, paths));
  let index = 0;
  for (const step of c.steps) {
    if (step.op === "write") {
      const file = join(paths.project, expand(step.path, paths));
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, expand(step.body, paths));
    } else if (step.op === "sh") {
      await sh(expand(step.script, paths), paths.project, environment(c, {}, paths));
    } else {
      check(step, await observe(c, step, paths), index);
      index += 1;
    }
  }
}
