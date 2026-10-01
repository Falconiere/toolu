#!/usr/bin/env bun
import { existsSync, readFileSync } from "node:fs";
import {
  atomicWriteJson,
  BabysitError,
  fail,
  flag,
  parseFlags,
  runCli,
  SlotLock,
  utcNow,
} from "./babysit/common";
import { collectPr } from "./babysit/collect";
import { reduceState } from "./babysit/reduce";

type Obj = Record<string, any>;

runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "babysit-tick.sh", [
    "--repo",
    "--pr",
    "--state-file",
    "--snapshot-out",
    "--snapshot-in",
    "--page-size",
    "--timeout",
    "--now",
  ]);
  const repo = flag(flags, "--repo");
  const numberText = flag(flags, "--pr");
  const stateFile = flag(flags, "--state-file");
  const snapshotIn = flag(flags, "--snapshot-in");
  const snapshotOut =
    flag(flags, "--snapshot-out") || `${stateFile.replace(/\.json$/, "")}.snapshot.json`;
  const pageText = flag(flags, "--page-size");
  const timeoutText = flag(flags, "--timeout");
  const now = flag(flags, "--now") || utcNow();
  if (!/^[^/\s]+\/[^/\s]+$/.test(repo))
    fail("usage", "babysit-tick.sh: --repo <owner/repo> required");
  if (!/^\d+$/.test(numberText)) fail("usage", "babysit-tick.sh: --pr <n> required");
  if (!stateFile) fail("usage", "babysit-tick.sh: --state-file <path> required");
  if (pageText && !/^\d+$/.test(pageText))
    fail("usage", "babysit-tick.sh: --page-size must be an integer");
  if (timeoutText && !/^\d+$/.test(timeoutText))
    fail("usage", "babysit-tick.sh: --timeout must be an integer");
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(now))
    fail("usage", "babysit-tick.sh: --now must be YYYY-MM-DDTHH:MM:SSZ");
  if (!snapshotIn && !Bun.which("gh")) fail("gh_unavailable", "gh is required");
  const number = Number(numberText);
  const lock = new SlotLock(stateFile);
  lock.acquire();
  try {
    let previous: Obj | null = null;
    if (existsSync(stateFile)) {
      try {
        previous = JSON.parse(readFileSync(stateFile, "utf8")) as Obj;
      } catch {
        fail("state_malformed", `babysit-tick.sh: state file is not valid JSON: ${stateFile}`, {
          source: "state",
        });
      }
      if (previous?.version !== 2)
        fail("state_malformed", `babysit-tick.sh: state file is not version 2: ${stateFile}`, {
          source: "state",
          version: previous?.version ?? null,
        });
      if (previous.repo !== repo || previous.number !== number) {
        fail(
          "slot_mismatch",
          `babysit-tick.sh: state file belongs to ${previous.repo}#${previous.number}, asked for ${repo}#${number}`,
          {
            source: "state",
            state: { repo: previous.repo, number: previous.number },
            requested: { repo, number },
          },
        );
      }
    }

    const stampFailure = (error: unknown): void => {
      if (previous === null) return;
      const code = error instanceof BabysitError ? error.code : "api_error";
      const message = error instanceof Error ? error.message : String(error);
      const pr = previous.pr as Obj;
      pr.lastError = { code, message, at: now };
      try {
        atomicWriteJson(stateFile, previous);
      } catch {
        process.stderr.write(`babysit-tick.sh: could not record lastError in ${stateFile}\n`);
      }
    };

    let snapshot: Obj;
    let rawSnapshot: string | undefined;
    if (snapshotIn) {
      try {
        rawSnapshot = readFileSync(snapshotIn, "utf8");
        snapshot = JSON.parse(rawSnapshot) as Obj;
      } catch {
        fail("invalid_json", `babysit-tick.sh: --snapshot-in is not a JSON file: ${snapshotIn}`, {
          source: "snapshot",
        });
      }
      if (snapshot!.repo !== repo || snapshot!.number !== number) {
        fail(
          "slot_mismatch",
          `babysit-tick.sh: --snapshot-in is for ${snapshot!.repo}#${snapshot!.number}, asked for ${repo}#${number}`,
          { source: "snapshot" },
        );
      }
    } else {
      try {
        snapshot = await collectPr({
          repo,
          pr: number,
          ...(pageText ? { pageSize: Number(pageText) } : {}),
          ...(timeoutText ? { timeoutSeconds: Number(timeoutText) } : {}),
        });
      } catch (error) {
        stampFailure(error);
        throw error;
      }
    }

    let reduced: ReturnType<typeof reduceState>;
    try {
      if (
        snapshot!.version !== 1 ||
        typeof snapshot!.repo !== "string" ||
        typeof snapshot!.number !== "number" ||
        typeof snapshot!.head?.sha !== "string" ||
        !snapshot!.pr ||
        typeof snapshot!.pr !== "object" ||
        !Array.isArray(snapshot!.threads)
      ) {
        fail("invalid_json", "reduce-state.sh: snapshot is not a version-1 pr-babysit snapshot", {
          source: "snapshot",
        });
      }
      reduced = reduceState(snapshot!, previous, now, stateFile, snapshotOut);
    } catch (error) {
      const failure =
        error instanceof BabysitError
          ? error
          : new BabysitError(
              "invalid_json",
              `reduce-state.sh: reducer failed on ${snapshotIn || snapshotOut}`,
              { source: "reduce" },
            );
      stampFailure(failure);
      throw failure;
    }
    try {
      atomicWriteJson(snapshotOut, snapshot!, rawSnapshot);
    } catch {
      fail("invalid_json", `babysit-tick.sh: could not write ${snapshotOut}`, {
        source: "snapshot_out",
      });
    }
    try {
      atomicWriteJson(stateFile, reduced.state);
    } catch {
      fail("invalid_json", `babysit-tick.sh: could not write ${stateFile}`, { source: "state" });
    }
    return reduced.result;
  } finally {
    lock.release();
  }
});
