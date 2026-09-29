/**
 * Uniform violation reporting for every guardrails check.
 *
 *   guardrails[<check-id>] <path>: <problem> — <remedy>
 *
 * The remedy clause is mandatory: these lines are read by an agent mid-turn
 * through a PostToolUse hook, and one told only what is wrong retries blindly.
 */

/** The guardrail itself is broken or misconfigured: always exit 3, never 1. */
export class GuardrailsFatal extends Error {}

export function fatal(message: string): never {
  throw new GuardrailsFatal(message);
}

/** Collects one run's outcome; each line goes to stderr the moment it is found. */
export class Reporter {
  failed = false;

  /**
   * `prefix` re-attaches a workspace package to a package-relative path, so the
   * violation reads `packages/database/src/foo.ts` — the path a person can open.
   */
  constructor(readonly prefix: string) {}

  violation(check: string, path: string, problem: string, remedy: string): void {
    process.stderr.write(`guardrails[${check}] ${this.prefix}${path}: ${problem} — ${remedy}\n`);
    this.failed = true;
  }
}

/** Advisory only; never affects the exit code. */
export function warn(message: string): void {
  process.stderr.write(`guardrails: warning: ${message}\n`);
}

export function printFatal(message: string): void {
  process.stderr.write(`guardrails: ${message}\n`);
}
