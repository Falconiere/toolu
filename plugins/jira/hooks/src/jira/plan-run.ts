/**
 * The plan check runner: probe Jira once, then run each step's `check` and
 * record the verdict. A step is green iff its check exits 0, and every check is
 * a live Jira REST assertion, so green means "Jira agrees".
 *
 * An infra failure is not a Jira disagreement: a missing credential or an
 * unreachable host would land every step red. So `user whoami` is probed
 * first and, on failure, the run aborts before writing anything.
 */
import { CliExit, writeStdout } from "@toolu/core/cli";
import { spawnSync } from "node:child_process";
import {
  accessSync,
  closeSync,
  constants,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import { constants as os, tmpdir } from "node:os";
import { join } from "node:path";
import type { Env } from "./creds.ts";
import { readFlags, unknownOption } from "./flags.ts";
import { nestedEnv } from "./opencode.ts";
import { parseSteps } from "./plan-parse.ts";
import {
  buildLedger,
  isCodex,
  type Ledger,
  ledgerPath,
  now,
  evidence,
  readLedger,
  repoRoot,
  setStep,
  writeLedger,
} from "./plan-store.ts";

/**
 * The jira CLI a check invokes as "$JIRA": JIRA_CLI, else the published path
 * under TOOLU_CONFIG_DIR, CODEX_HOME (Codex) or CLAUDE_CONFIG_DIR.
 */
export function planCli(env: Env): string {
  if (env["JIRA_CLI"]) return env["JIRA_CLI"];
  const home = env["HOME"] ?? "";
  const root =
    env["TOOLU_CONFIG_DIR"] ||
    (isCodex(env)
      ? env["CODEX_HOME"] || join(home, ".codex")
      : env["CLAUDE_CONFIG_DIR"] || join(home, ".claude"));
  return join(root, "jira", "jira.sh");
}

/** bash `[ -x ] || [ -f ]`. */
function runnable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return statSync(path, { throwIfNoEntry: false })?.isFile() === true;
  }
}

/** The exit status bash would report: the code, or 128 + the signal number. */
function statusOf(run: ReturnType<typeof spawnSync>): number {
  if (run.status !== null) return run.status;
  const signal = run.signal === null ? undefined : os.signals[run.signal];
  return 128 + (signal ?? 0);
}

/** `"$cli" user whoami` with its output discarded: zero when Jira answers. */
function probe(cli: string, env: Env): boolean {
  const run = spawnSync(cli, ["user", "whoami"], { env, stdio: ["inherit", "ignore", "ignore"] });
  return run.error === undefined && run.status === 0;
}

/**
 * Runs `check` with `bash -c` in `root`, `$JIRA` bound to `cli`. Stdout and
 * stderr share one file, as `2>&1` did; trailing newlines are dropped as
 * `$(…)` dropped them.
 */
function runCheck(
  cli: string,
  check: string,
  root: string,
  env: Env,
): { out: string; code: number } {
  const dir = mkdtempSync(join(tmpdir(), "jira-plan-"));
  try {
    const file = join(dir, "out");
    const fd = openSync(file, "w");
    let run: ReturnType<typeof spawnSync>;
    try {
      run = spawnSync("bash", ["-c", check], {
        cwd: root,
        env: { ...env, JIRA: cli },
        stdio: ["inherit", fd, fd],
      });
    } finally {
      closeSync(fd);
    }
    if (run.error !== undefined) return { out: `jira plan: ${run.error.message}`, code: 1 };
    return { out: readFileSync(file, "utf8").replace(/\n+$/, ""), code: statusOf(run) };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

interface RunContext {
  readonly key: string;
  readonly doc: string;
  /** What plan checks inherit, including the resolved _JIRA_VER / _JIRA_LEAN. */
  readonly env: Env;
  readonly cwd: string;
}

function runOptions(argv: readonly string[]): { only: string; activity: string } {
  const flags = readFlags(argv, { values: { "--step": "step", "--activity": "activity" } }, (arg) =>
    unknownOption("jira plan run", arg),
  );
  return { only: flags.values.get("step") ?? "", activity: flags.values.get("activity") ?? "" };
}

async function runStep(
  ledger: Ledger,
  id: string,
  state: string,
  context: { cli: string; root: string; env: Env; activity: string },
): Promise<{ ledger: Ledger; green: boolean }> {
  const check = String(ledger.steps.find((step) => step["id"] === id)?.["check"] ?? "");
  const running = { status: "running", started_at: now(), exit_code: null, evidence_tail: null };
  let current = setStep(
    ledger,
    id,
    context.activity === "" ? running : { ...running, activity: context.activity },
  );
  // First write: the dashboard shows the step in flight while its check runs.
  writeLedger(state, current);
  const { out, code } = runCheck(context.cli, check, context.root, context.env);
  current = setStep(current, id, {
    status: code === 0 ? "green" : "red",
    exit_code: code,
    last_run: now(),
    evidence_tail: evidence(out),
    started_at: null,
  });
  writeLedger(state, current);
  await writeStdout(code === 0 ? `green  ${id}\n` : `red    ${id} (exit ${code})\n`);
  return { ledger: current, green: code === 0 };
}

/**
 * Merges the doc's steps over the persisted ledger, probes Jira, then runs
 * one step (--step) or all of them. Exits 1 when any executed step is red.
 */
export async function runPlan(context: RunContext, argv: readonly string[]): Promise<number> {
  const { only, activity } = runOptions(argv);
  const steps = parseSteps(context.doc);
  const root = repoRoot(context.cwd);
  const state = ledgerPath(context.key, context.env, context.cwd);
  let ledger = buildLedger(context.key, context.doc, steps, readLedger(state));
  if (only !== "" && !ledger.steps.some((step) => step["id"] === only)) {
    throw new CliExit(1, `jira plan run: no step '${only}' in ${context.doc}`);
  }
  const cli = planCli(context.env);
  if (!runnable(cli))
    throw new CliExit(1, `jira plan run: jira CLI not found at ${cli} (set JIRA_CLI)`);
  const env = nestedEnv(context.env);
  // Probe BEFORE the first write: an unreachable Jira must not manufacture reds.
  if (!probe(cli, env)) {
    throw new CliExit(
      1,
      "jira plan run: cannot reach Jira (user whoami failed) — no step statuses were written",
    );
  }
  const targets = only === "" ? ledger.steps.map((step) => String(step["id"])) : [only];
  let failed = false;
  for (const id of targets) {
    const result = await runStep(ledger, id, state, { cli, root, env, activity });
    ledger = result.ledger;
    failed ||= !result.green;
  }
  return failed ? 1 : 0;
}
