/** Run an expensive test/gate command with the worktree's shared job admission. */
import { resourceBinding } from "../hooks/dist/epic-runtime.js";
import { runManagedJob } from "../hooks/dist/epic-runtime.js";
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

function waitingFile(root: string, key: string): string {
  const safe = [...key].filter((letter) => /[A-Za-z0-9-]/.test(letter)).slice(0, 80).join("");
  return join(root, "waiting-jobs", `${safe}.json`);
}

function markWaiting(binding: NonNullable<ReturnType<typeof resourceBinding>>): void {
  const path = waitingFile(binding.root, binding.key);
  mkdirSync(join(binding.root, "waiting-jobs"), { recursive: true, mode: 0o700 });
  let firstWaitAt = new Date().toISOString();
  try {
    const earlier = JSON.parse(readFileSync(path, "utf8")) as { first_wait_at?: string };
    if (earlier.first_wait_at && !Number.isNaN(Date.parse(earlier.first_wait_at)))
      firstWaitAt = earlier.first_wait_at;
  } catch { /* A missing or damaged marker starts a new wait. */ }
  const body = JSON.stringify({
    schema: 1, key: binding.key, epic: binding.stateDir.split("/").filter(Boolean).at(-1) ?? "",
    worktree: binding.worktree, first_wait_at: firstWaitAt,
    last_attempt_at: new Date().toISOString(),
  });
  const temporary = `${path}.${process.pid}.tmp`;
  try {
    writeFileSync(temporary, `${body}\n`, { flag: "wx", mode: 0o600 });
    renameSync(temporary, path);
  } catch (error) {
    rmSync(temporary, { force: true });
    throw error;
  }
}

async function main(): Promise<void> {
  const [separator, ...argv] = process.argv.slice(2);
  if (separator !== "--" || !argv.length) throw new Error("usage: job.ts -- COMMAND [ARG ...]");
  const binding = resourceBinding(process.cwd());
  if (!binding) throw new Error("worktree has no epic resource binding");
  let result;
  try {
    result = await runManagedJob(argv, binding, { cwd: process.cwd(), timeoutMs: 3_600_000 });
    rmSync(waitingFile(binding.root, binding.key), { force: true });
  } catch (error) {
    if (/job capacity exhausted|resource hold:/.test(String(error))) markWaiting(binding);
    else rmSync(waitingFile(binding.root, binding.key), { force: true });
    throw error;
  }
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  process.exit(result.timedOut ? 124 : result.exitCode);
}
if (import.meta.main)
  main().catch((error: unknown) => {
    process.stderr.write(`${String(error)}\n`);
    process.exit(1);
  });
