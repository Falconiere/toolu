/**
 * A real runner process for the ledger-check test (#256). It runs one
 * long-lived check through `runCheck` and records the check's pid, so the
 * test can kill this runner and prove the check's group dies with it.
 * Usage: bun run run-check-runner.ts <cwd> <pid-file>
 */
import { join } from "node:path";
import { runCheck } from "../ledger-check.ts";

const [cwd = ".", pidFile = "check.pid"] = process.argv.slice(2);
await runCheck({
  check: `echo $$ > '${pidFile}'; exec sleep 47`,
  cwd,
  env: { PATH: process.env.PATH ?? "" },
  outFile: join(cwd, "runner-out.txt"),
  timeout: "1800",
});
