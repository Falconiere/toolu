#!/usr/bin/env bun
/** Read one coherent PR snapshot and publish it atomically. */
import { atomicWriteJson, fail, flag, parseFlags, runCli } from "./babysit/common";
import { collectPr } from "./babysit/collect";

runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "collect-pr.sh", [
    "--repo",
    "--pr",
    "--out",
    "--page-size",
    "--timeout",
  ]);
  const repo = flag(flags, "--repo");
  const prText = flag(flags, "--pr");
  const out = flag(flags, "--out");
  const pageText = flag(flags, "--page-size") || "100";
  const timeoutText = flag(flags, "--timeout") || process.env.PB_GH_TIMEOUT || "60";
  if (!/^[^/\s]+\/[^/\s]+$/.test(repo))
    fail("usage", "collect-pr.sh: --repo <owner/repo> required");
  if (!/^\d+$/.test(prText)) fail("usage", "collect-pr.sh: --pr <n> required");
  if (!out) fail("usage", "collect-pr.sh: --out <path> required");
  if (!/^\d+$/.test(pageText) || Number(pageText) < 1)
    fail("usage", "collect-pr.sh: --page-size must be a positive integer");
  if (!/^\d+$/.test(timeoutText) || Number(timeoutText) < 1)
    fail("usage", "collect-pr.sh: --timeout must be a positive integer");
  if (!Bun.which("gh")) fail("gh_unavailable", "gh is required");
  const snapshot = await collectPr({
    repo,
    pr: Number(prText),
    pageSize: Number(pageText),
    timeoutSeconds: Number(timeoutText),
  });
  try {
    atomicWriteJson(out, snapshot);
  } catch {
    fail("invalid_json", "collect-pr.sh: could not assemble the snapshot", { source: "snapshot" });
  }
  process.stdout.write(`${out}\n`);
});
