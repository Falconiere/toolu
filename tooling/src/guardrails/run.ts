/**
 * run.ts — the agent-guardrails entry point (`bun run guardrails`).
 *
 * Enforces the structural rules a linter cannot see: the folder tree,
 * per-domain shape, colocated tests, barrels, banned dependencies, required
 * files, committed secrets, contextual code patterns, and attempts to suppress
 * dead-code enforcement. Modes and flags: see cli.ts.
 *
 * Exit: 0 clean · 1 violations · 2 violations in --hook/--stop · 3 misconfigured.
 * Exit 2 in the hook modes is load-bearing: Claude Code ignores a 1 from a
 * hook; only 2 shows stderr on PostToolUse and only 2 blocks on Stop.
 */
import { parseArgs } from "./cli.ts";
import { ALL_CHECK_IDS } from "./registry.ts";
import { GuardrailsFatal, printFatal } from "./report.ts";
import { runSingle } from "./single.ts";
import { isWorkspace, runWorkspace } from "./workspace.ts";

function main(argv: readonly string[]): number {
  try {
    const opts = parseArgs(argv);
    if (opts.list) {
      process.stdout.write(`${ALL_CHECK_IDS.join("\n")}\n`);
      return 0;
    }
    const cwd = process.cwd();
    return isWorkspace(cwd) ? runWorkspace(cwd, opts) : runSingle(cwd, opts);
  } catch (err: unknown) {
    if (err instanceof GuardrailsFatal) printFatal(err.message);
    else printFatal(`unexpected failure: ${err instanceof Error ? err.message : String(err)}`);
    return 3;
  }
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
