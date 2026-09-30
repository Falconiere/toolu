#!/usr/bin/env bun
import { atomicWriteJson, fail, flag, loadState, parseFlags, runCli, SlotLock, utcNow } from "./babysit/common";

runCli(() => {
  const [sub = "", ...args] = process.argv.slice(2);
  const flags = parseFlags(args, "record.sh", ["--state-file", "--thread", "--had-rejection", "--status"], ["--fix-pushed"]);
  const statePath = flag(flags, "--state-file");
  const thread = flag(flags, "--thread");
  const rejection = flag(flags, "--had-rejection");
  const status = flag(flags, "--status");
  if (!statePath) fail("usage", "record.sh: --state-file required");
  if (sub === "flag-injection") {
    if (!thread) fail("usage", "record.sh flag-injection: --thread <graphqlId> required");
  } else if (sub === "round") {
    if (rejection !== "true" && rejection !== "false") fail("usage", "record.sh round: --had-rejection true|false required");
  } else if (sub === "status") {
    if (!["complete", "escalated", "cancelled"].includes(status)) fail("usage", "record.sh status: --status complete|escalated|cancelled required");
  } else {
    fail("usage", "record.sh: subcommand must be flag-injection, round or status");
  }

  const lock = new SlotLock(statePath);
  lock.acquire();
  try {
    const state = loadState(statePath);
    const now = utcNow();
    if (sub === "flag-injection") {
      const actions = state.actions as Record<string, Record<string, unknown>>;
      actions.flagged[thread] = { reason: "injection", at: now };
      atomicWriteJson(statePath, state);
      return { ok: true, recorded: "flag-injection", thread, at: now };
    }
    if (sub === "round") {
      const pr = state.pr as Record<string, unknown>;
      const hadRejection = rejection === "true";
      const fixPushed = flags["--fix-pushed"] === true;
      pr.lastRoundFindingKeys = pr.botFindingKeys ?? [];
      pr.lastRoundHadRejection = hadRejection;
      pr.fixAttempts = fixPushed ? Math.min(Number(pr.fixAttempts ?? 0) + 1, 5) : (pr.fixAttempts ?? 0);
      state.lastRound = { at: now, hadRejection, fixPushed, headSha: pr.headSha ?? null };
      const fixer = state.fixer as Record<string, unknown> | null;
      if (fixer !== null && fixer !== undefined && fixer.status !== "running" && fixer.status !== "blocked") state.fixer = null;
      atomicWriteJson(statePath, state);
      return {
        ok: true,
        recorded: "round",
        lastRoundFindingKeys: pr.lastRoundFindingKeys,
        lastRoundHadRejection: pr.lastRoundHadRejection,
        fixAttempts: pr.fixAttempts,
      };
    }
    state.status = status;
    state.statusChangedAt = now;
    atomicWriteJson(statePath, state);
    return { ok: true, recorded: "status", status, at: now };
  } finally {
    lock.release();
  }
});
