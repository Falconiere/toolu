#!/usr/bin/env bun
import { fail, flag, parseFlags, runCli } from "./babysit/common";
import { resolveThread } from "./babysit/writes";

runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "resolve-thread.sh", [
    "--state-file",
    "--thread",
    "--timeout",
  ]);
  const statePath = flag(flags, "--state-file");
  const thread = flag(flags, "--thread");
  if (!statePath) fail("usage", "resolve-thread.sh: --state-file required");
  if (!/^[A-Za-z0-9_=-]+$/.test(thread))
    fail("usage", `resolve-thread.sh: --thread <graphqlId> required (got '${thread}')`);
  if (!Bun.which("gh")) fail("gh_unavailable", "gh is required");
  return resolveThread({
    statePath,
    thread,
    ...(flag(flags, "--timeout") ? { timeoutSeconds: Number(flag(flags, "--timeout")) } : {}),
  });
});
