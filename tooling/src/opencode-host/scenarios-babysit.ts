/**
 * Pinned-host pr-babysit (#357): an OpenCode controller runs the shipped
 * helpers through its own bash, and `babysit-dispatch-fix.js` starts a real
 * OpenCode fixer (`opencode run`) in a native worktree of an isolated
 * repository whose PR branch has a failing test. Both sessions talk to the
 * scripted provider: the controller by its `PROBE:` token, the fixer through
 * the `*` script.
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  BRANCH,
  RUN_MS,
  SKILL,
  SLEEP,
  SLOT,
  babysitSession,
  bash,
  firstGroup,
  fixerSteps,
  fixerTools,
  git,
  helper,
  json,
  originOf,
  paths,
  reap,
  refused,
  running,
  startSteps,
  text,
  waitStep,
} from "./babysit-fixture.ts";
import { runHost } from "./host-run.ts";
import { toolResults } from "./install-host.ts";
import {
  diagnostics,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";

/** What the fixer's remote writes and subagent calls came to, from its own log. */
function writeGuards(calls: ReturnType<typeof fixerTools>): Record<string, boolean> {
  const said = (needle: string, pattern: RegExp): boolean =>
    calls.some((c) => c.input.includes(needle) && pattern.test(`${c.output}${c.error}`));
  return {
    pushDenied: refused(calls, "bash", `"git push origin HEAD:${BRANCH}"`),
    gitCDenied: refused(calls, "bash", `' push origin HEAD:${BRANCH}`),
    ghDenied: refused(calls, "bash", "gh pr comment"),
    // Forms no deny pattern names: the fixer's environment stops them.
    wrappedPushBlocked: said(`"env git push origin HEAD:${BRANCH}"`, /pr-babysit-fixer-no-push/),
    ghNoLogin: said('"env gh auth status"', /not logged in/i),
    // A denied `task` is not even offered: the host turns the call into `invalid`.
    taskUnavailable: calls.some(
      (c) => c.tool === "invalid" && c.input.includes("unavailable tool 'task'"),
    ),
  };
}

async function fixer(ctx: EntryContext): Promise<EntryResult> {
  using s = babysitSession(ctx, (q) => ({
    "babysit.fixer": [
      { tool: "skill", args: { name: SKILL } },
      ...startSteps(q),
      waitStep(q, "wait"),
      bash(
        `cd '${q.worktree}' && "$TOOLU_BUN" test sum.test.ts > '${q.project}/verify.txt' 2>&1; printf 'verify=%s\\n' "$?" >> '${q.project}/exits.txt'`,
      ),
      helper(
        "record",
        `babysit-record.js" round --state-file '${q.state}' --had-rejection false --fix-pushed`,
      ),
    ],
    "*": fixerSteps(q),
  }));
  const p = paths(s.sb.project);
  try {
    const { path: originPath, prHead } = originOf(s);
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:babysit.fixer"], RUN_MS);
    const state = json(p.state);
    const wait = json(join(p.project, "wait.json"));
    const calls = fixerTools(text(p.log));
    const commits = Array.isArray(wait["commits"]) ? wait["commits"] : [];
    const observed = {
      ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
      skill: toolResults(hostRun.events).some(
        (r) => r.tool === "skill" && r.status === "completed",
      ),
      exits:
        text(join(p.project, "exits.txt")) ===
        "tick=0\nroute=0\nstart=0\nwait=0\nverify=0\nrecord=0\n",
      routed: JSON.stringify(json(join(p.project, "route.json"))["groups"] ?? "").includes(
        '"host":"opencode","model":"probe/scripted"',
      ),
      done:
        wait["status"] === "done" && firstGroup(wait)?.status === "done" && commits.length === 1,
      testsPass: /\b1 pass\b/.test(text(join(p.project, "verify.txt"))),
      onlySeeded:
        existsSync(p.worktree) &&
        git(p.worktree, "diff", "--name-only", `origin/${BRANCH}..HEAD`) === "sum.ts",
      originUnchanged: git(originPath, "rev-parse", BRANCH) === prHead,
      ...writeGuards(calls),
      reported: json(p.report)["status"] === "done",
      stateHere: state["version"] === 2 && state["slot"] === SLOT,
      recorded:
        state["fixer"] === null && JSON.stringify(state["pr"] ?? "").includes('"fixAttempts":1'),
    };
    return { pass: Object.values(observed).every(Boolean), observed };
  } finally {
    reap(p);
  }
}

async function noReport(ctx: EntryContext): Promise<EntryResult> {
  using s = babysitSession(ctx, (q) => ({
    "babysit.no-report": [
      ...startSteps(q),
      waitStep(q, "wait"),
      waitStep(q, "again"),
      helper("cleanup", `babysit-dispatch-fix.js" cleanup --state-file '${q.state}'`),
    ],
  }));
  const p = paths(s.sb.project);
  try {
    await runHost(ctx.bin, s, ["--print-logs", "PROBE:babysit.no-report"], RUN_MS);
    const wait = json(join(p.project, "wait.json"));
    const again = json(join(p.project, "again.json"));
    const cleanup = json(join(p.project, "cleanup.json"));
    const state = json(p.state);
    const observed = {
      exits:
        text(join(p.project, "exits.txt")) ===
        "tick=0\nroute=0\nstart=0\nwait=0\nagain=0\ncleanup=0\n",
      failed: wait["status"] === "failed" && wait["reason"] === "no_report",
      logged: /exited without a report; last output: \S/.test(firstGroup(wait)?.error ?? ""),
      stable: again["status"] === "failed" && again["reason"] === "no_report",
      cleaned: cleanup["status"] === "cleaned" && cleanup["worktreeRemoved"] === true,
      worktreeGone: !existsSync(p.worktree),
      ledgerUntouched:
        JSON.stringify(state["actions"]) ===
        JSON.stringify({ replied: {}, resolved: {}, flagged: {} }),
      noReport: !existsSync(p.report),
    };
    return { pass: Object.values(observed).every(Boolean), observed };
  } finally {
    reap(p);
  }
}

async function cancel(ctx: EntryContext): Promise<EntryResult> {
  using s = babysitSession(ctx, (q) => ({
    "babysit.cancel": [
      ...startSteps(q),
      bash(`sleep 20; cp '${q.state}' running.json`),
      helper("cleanup", `babysit-dispatch-fix.js" cleanup --state-file '${q.state}'`),
      helper("status", `babysit-record.js" status --state-file '${q.state}' --status cancelled`),
    ],
    "*": [bash(SLEEP, 200_000)],
  }));
  const p = paths(s.sb.project);
  try {
    await runHost(ctx.bin, s, ["--print-logs", "PROBE:babysit.cancel"], RUN_MS);
    const before = firstGroup(json(join(p.project, "running.json")));
    const pid = before?.pid ?? 0;
    const groupLeft = spawnSync("ps", ["-A", "-o", "pgid="], { encoding: "utf8" })
      .stdout.split("\n")
      .filter((line) => line.trim() === String(pid)).length;
    const state = json(p.state);
    const observed = {
      exits:
        text(join(p.project, "exits.txt")) === "tick=0\nroute=0\nstart=0\ncleanup=0\nstatus=0\n",
      wasRunning: before?.status === "running" && before.host === "opencode" && pid > 1,
      groupGone: groupLeft === 0,
      sleepGone: running(SLEEP) === 0,
      cleaned: json(join(p.project, "cleanup.json"))["worktreeRemoved"] === true,
      worktreeGone: !existsSync(p.worktree),
      fixerCleared: state["fixer"] === null,
      cancelled: state["status"] === "cancelled",
      ledgerUntouched:
        JSON.stringify(state["actions"]) ===
        JSON.stringify({ replied: {}, resolved: {}, flagged: {} }),
    };
    return { pass: Object.values(observed).every(Boolean), observed };
  } finally {
    reap(p);
  }
}

export const BABYSIT_SCENARIOS: EntryScenario[] = [
  {
    id: "babysit.fixer",
    claim:
      "An OpenCode controller dispatches an OpenCode fixer that fixes a seeded failing test; its remote writes and subagents are denied",
    run: fixer,
  },
  {
    id: "babysit.no-report",
    claim: "A fixer that ends without a report settles no_report with its last output",
    run: noReport,
  },
  {
    id: "babysit.cancel",
    claim: "Cancel ends a running fixer's processes, removes its worktree and records cancelled",
    run: cancel,
  },
];
