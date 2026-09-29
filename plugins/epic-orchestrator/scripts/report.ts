/** Worker status report for the epic orchestrator.
 *
 * Usage: bun report.ts <status-file> <phase> [--pr N] [--note "text"]
 * Writes {phase, pr, note, updated_at, history[]} atomically; history keeps
 * every transition, and keys a previous status carried keep their place. */

import { readJson, writeJson } from "./common.ts";

const PHASES = [
  "brainstorm",
  "spec",
  "spec-review",
  "plan",
  "plan-review",
  "execution",
  "pr-open",
  "babysit",
  "rebasing",
  "ready",
  "needs-human",
  "failed",
] as const;

const USAGE = `usage: report.ts <status-file> <phase> [--pr N] [--note text]; phases: ${PHASES.join(" ")}`;

class UsageError extends Error {}

type Args = { file: string; phase: string; pr: string; note: string };

function parseArgs(argv: string[]): Args {
  const [file, phase, ...rest] = argv;
  if (file === undefined || phase === undefined) throw new UsageError(USAGE);
  if (!(PHASES as readonly string[]).includes(phase)) {
    throw new UsageError(`unknown phase: ${phase}\n${USAGE}`);
  }
  let pr = "";
  let note = "";
  for (let i = 0; i < rest.length; i += 2) {
    const flag = rest[i];
    const value = rest[i + 1];
    if (flag === "--pr") {
      if (!value) throw new UsageError("--pr needs a number");
      pr = value;
    } else if (flag === "--note") {
      if (!value) throw new UsageError("--note needs text");
      note = value;
    } else {
      throw new UsageError(USAGE);
    }
  }
  if (pr !== "" && !/^\d+$/.test(pr)) throw new UsageError("--pr must be a number");
  return { file, phase, pr, note };
}

type History = { phase: string; at: string; note: string | null }[];

function previous(file: string): Record<string, unknown> {
  const prev = readJson<unknown>(file, {});
  return typeof prev === "object" && prev !== null && !Array.isArray(prev) ? { ...prev } : {};
}

async function report(args: Args): Promise<string> {
  const status = previous(args.file);
  const now = `${new Date().toISOString().slice(0, 19)}Z`;
  const note = args.note === "" ? null : args.note;
  const history = Array.isArray(status.history) ? (status.history as History) : [];
  status.phase = args.phase;
  status.pr = args.pr === "" ? (status.pr ?? null) : Number(args.pr);
  status.note = note;
  status.updated_at = now;
  status.history = [...history, { phase: args.phase, at: now, note }];
  await writeJson(args.file, status);
  return `reported ${args.phase}${args.pr === "" ? "" : ` (PR #${args.pr})`}`;
}

async function main(): Promise<void> {
  process.stdout.write(`${await report(parseArgs(process.argv.slice(2)))}\n`);
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(`${err instanceof Error ? err.message : String(err)}\n`);
    process.exit(err instanceof UsageError ? 2 : 1);
  });
}
