/**
 * Running one plan step's `check` (#256): `pl_run_check` and `pl_evidence`.
 * The check runs as `bash -c` in its own process group with stdin on
 * /dev/null, and stdout and stderr go interleaved to one file. The
 * `PLAN_LEDGER_STEP_TIMEOUT` bound is enforced natively (decision D1): at the
 * deadline the whole group gets SIGTERM, then SIGKILL after a grace period,
 * and the step exits 124 as GNU `timeout` reports it.
 */
import { closeSync, openSync } from "node:fs";
import { constants } from "node:os";
import { toJqJson } from "../state/state-io.ts";

export const TIMEOUT_EXIT = 124;
const INVALID_TIMEOUT_EXIT = 125;
const KILL_GRACE_MS = 2000;
const EVIDENCE_LINES = 10;
const EVIDENCE_BYTES = 2000;

const UNIT_SECONDS: Record<string, number> = { "": 1, s: 1, m: 60, h: 3600, d: 86400 };

/** The longest delay `setTimeout` honours; anything larger overflows to 1 ms. */
const MAX_DELAY_MS = 2_147_483_647;

/**
 * A GNU `timeout` duration in seconds (`1800`, `+1.5`, ` 1e3`, `2m`); 0
 * disables the bound. Undefined for text outside that decimal grammar. The
 * hex, `inf` and `nan` forms that strtod also takes are not supported and read
 * as invalid.
 */
export function parseTimeout(value: string): number | undefined {
  const match = /^[ \t\n\v\f\r]*\+?((?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)([smhd]?)$/.exec(value);
  if (match === null) return undefined;
  const seconds = Number(match[1]) * (UNIT_SECONDS[match[2] ?? ""] ?? 1);
  return Number.isFinite(seconds) ? seconds : undefined;
}

export type CheckRun = {
  check: string;
  cwd: string;
  env: Record<string, string>;
  /** Combined stdout+stderr lands here; the caller reads and removes it. */
  outFile: string;
  /** The raw `PLAN_LEDGER_STEP_TIMEOUT` text ("0" disables). */
  timeout: string;
};

function killGroup(pid: number, signal: NodeJS.Signals): void {
  try {
    process.kill(-pid, signal);
  } catch {
    // The group is already gone: nothing left to stop.
  }
}

const FORWARDED: readonly NodeJS.Signals[] = ["SIGINT", "SIGTERM", "SIGHUP"];

/**
 * While a check runs, a runner that exits or is signalled takes the check's
 * process group down with it. Otherwise a detached check would outlive it,
 * unbounded. The signal is then re-raised so the runner still dies as it
 * would have. Returns the release function.
 */
function guardGroup(pid: number): () => void {
  const onExit = (): void => killGroup(pid, "SIGKILL");
  const onSignal = (signal: NodeJS.Signals): void => {
    killGroup(pid, "SIGKILL");
    release();
    process.kill(process.pid, signal);
  };
  const release = (): void => {
    process.off("exit", onExit);
    for (const signal of FORWARDED) process.off(signal, onSignal);
  };
  process.on("exit", onExit);
  for (const signal of FORWARDED) process.on(signal, onSignal);
  return release;
}

/** Exit code as bash reports it: 128+N for a death by signal N. */
function exitCodeOf(proc: Bun.Subprocess): number {
  if (proc.exitCode !== null) return proc.exitCode;
  const signal = proc.signalCode;
  const number = signal === null ? undefined : constants.signals[signal];
  return 128 + (number ?? 0);
}

/** `pl_run_check`: the check's exit code (124 when the bound killed it). */
export async function runCheck(run: CheckRun): Promise<number> {
  const seconds = parseTimeout(run.timeout);
  const fd = openSync(run.outFile, "w");
  try {
    if (seconds === undefined) {
      const note = `plan-ledger: invalid PLAN_LEDGER_STEP_TIMEOUT '${run.timeout}'\n`;
      await Bun.write(run.outFile, note);
      return INVALID_TIMEOUT_EXIT;
    }
    const proc = Bun.spawn(["bash", "-c", run.check], {
      cwd: run.cwd,
      env: run.env,
      stdin: "ignore",
      stdout: fd,
      stderr: fd,
      detached: true,
    });
    const release = guardGroup(proc.pid);
    try {
      return await waitBounded(proc, seconds);
    } finally {
      release();
    }
  } finally {
    closeSync(fd);
  }
}

/** Wait for the check; past the bound, stop its group and report 124. */
async function waitBounded(proc: Bun.Subprocess, seconds: number): Promise<number> {
  if (seconds === 0) {
    await proc.exited;
    return exitCodeOf(proc);
  }
  let deadline: ReturnType<typeof setTimeout> | undefined;
  const expired = new Promise<true>((resolveExpired) => {
    deadline = setTimeout(() => resolveExpired(true), Math.min(seconds * 1000, MAX_DELAY_MS));
  });
  const timedOut = await Promise.race([proc.exited.then(() => false), expired]);
  clearTimeout(deadline);
  if (timedOut) {
    killGroup(proc.pid, "SIGTERM");
    const grace = setTimeout(() => killGroup(proc.pid, "SIGKILL"), KILL_GRACE_MS);
    await proc.exited;
    clearTimeout(grace);
    killGroup(proc.pid, "SIGKILL");
    return TIMEOUT_EXIT;
  }
  return exitCodeOf(proc);
}

/** `$(...)` capture: NUL bytes dropped, trailing newlines stripped. */
export function commandSubstitution(bytes: Uint8Array): Uint8Array {
  const kept = bytes.filter((b) => b !== 0);
  let end = kept.length;
  while (end > 0 && kept[end - 1] === 0x0a) end -= 1;
  return kept.subarray(0, end);
}

/** `tail -n 10` of text with no trailing newline. */
function lastLines(bytes: Uint8Array): Uint8Array {
  let seen = 0;
  for (let i = bytes.length - 1; i >= 0; i -= 1) {
    if (bytes[i] === 0x0a) {
      seen += 1;
      if (seen === EVIDENCE_LINES) return bytes.subarray(i + 1);
    }
  }
  return bytes;
}

/** `pl_evidence`: tail, cap and decode the output the way `tail | head -c | jq -Rs` does. */
export function evidenceOf(bytes: Uint8Array): string {
  const tail = lastLines(commandSubstitution(bytes)).subarray(0, EVIDENCE_BYTES);
  return new TextDecoder("utf-8").decode(tail);
}

/**
 * The evidence recorded for a finished check. On exit 124, bash prepends
 * the timeout reason to the already JSON-encoded tail and encodes it again.
 * That double encoding is reproduced here.
 */
export function stepEvidence(exitCode: number, output: Uint8Array, timeout: string): string {
  const evidence = evidenceOf(output);
  if (exitCode !== TIMEOUT_EXIT) return evidence;
  const reason = `timed out after ${timeout}s (PLAN_LEDGER_STEP_TIMEOUT)\n${toJqJson(evidence, false)}`;
  return evidenceOf(new TextEncoder().encode(reason));
}
