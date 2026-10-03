/** Run an expensive test/gate command with the worktree's shared job admission. */
import { resourceBinding } from "../hooks/dist/epic-runtime.js";
import { runManagedJob } from "../hooks/dist/epic-runtime.js";

async function main(): Promise<void> {
  const [separator, ...argv] = process.argv.slice(2);
  if (separator !== "--" || !argv.length) throw new Error("usage: job.ts -- COMMAND [ARG ...]");
  const binding = resourceBinding(process.cwd());
  if (!binding) throw new Error("worktree has no epic resource binding");
  const result = await runManagedJob(argv, binding, { cwd: process.cwd(), timeoutMs: 3_600_000 });
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  process.exit(result.timedOut ? 124 : result.exitCode);
}
if (import.meta.main)
  main().catch((error: unknown) => {
    process.stderr.write(`${String(error)}\n`);
    process.exit(1);
  });
