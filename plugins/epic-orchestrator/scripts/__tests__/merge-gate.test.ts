/** Merge-gate pure logic on real GitHub payloads. */

import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  PROTECTION,
  autoMergeAction,
  autoMergeArgs,
  autoMergeState,
  checkBuckets,
} from "../merge-gate.ts";
import { tickBody } from "../trackers/github.ts";

const FIX = join(import.meta.dir, "..", "fixtures");

const TITLE = "Define shared TS core contracts and verify OpenCode capabilities";

test.concurrent("checkBuckets: all green rollup", () => {
  const rollup = JSON.parse(readFileSync(join(FIX, "pr290-rollup.json"), "utf8")) as {
    name: string;
  }[];
  const buckets = checkBuckets(rollup);
  expect(buckets.pass?.length).toBe(rollup.length);
  expect(buckets.fail).toEqual([]);
  expect(buckets.pending).toEqual([]);
});

test.concurrent("checkBuckets: failed and running runs are not green", () => {
  const rollup = JSON.parse(readFileSync(join(FIX, "pr290-rollup.json"), "utf8")) as Record<
    string,
    unknown
  >[];
  const first = rollup[0];
  const second = rollup[1];
  if (!first || !second) throw new Error("rollup too short");
  rollup[0] = { ...first, conclusion: "FAILURE" };
  rollup[1] = { ...second, status: "IN_PROGRESS", conclusion: "" };
  const buckets = checkBuckets(rollup);
  expect(buckets.fail).toEqual([String(first.name)]);
  expect(buckets.pending).toEqual([String(second.name)]);
});

test.concurrent("checkBuckets: status context states", () => {
  expect(
    checkBuckets([
      { __typename: "StatusContext", context: "ci/a", state: "SUCCESS" },
      { __typename: "StatusContext", context: "ci/b", state: "PENDING" },
      { __typename: "StatusContext", context: "ci/c", state: "ERROR" },
    ]),
  ).toEqual({ pass: ["ci/a"], pending: ["ci/b"], fail: ["ci/c"] });
});

/** Lines of `body` that differ from `next`, as [before, after] pairs. */
function changedLines(body: string, next: string): [string, string][] {
  return body
    .split("\n")
    .map((a, i) => [a, next.split("\n")[i] ?? ""] as [string, string])
    .filter(([a, b]) => a !== b);
}

test.concurrent("tickBody ticks exactly the issue line", () => {
  const body = readFileSync(join(FIX, "epic248-body.md"), "utf8");
  const next = tickBody(body, ["Falconiere", "comemory"], "Falconiere/comemory#255");
  const changed = changedLines(body, next);
  expect(changed.length).toBe(1);
  const line = changed[0];
  if (!line) throw new Error("no change");
  expect(line[1].startsWith("- [x] https://github.com/Falconiere/comemory/issues/255 ")).toBe(true);
});

test.concurrent("tickBody cross-repo issue line", () => {
  const body = readFileSync(join(FIX, "epic248-body.md"), "utf8");
  const next = tickBody(body, ["Falconiere", "comemory"], "CodaSignal/comemory.io#183");
  expect(changedLines(body, next).length).toBe(1);
});

test.concurrent("tickBody prefix number does not match", () => {
  const body = readFileSync(join(FIX, "epic248-body.md"), "utf8");
  expect(tickBody(body, ["Falconiere", "comemory"], "Falconiere/comemory#25")).toBe(body);
});

test.concurrent("tickBody table mentions are untouched", () => {
  const body = readFileSync(join(FIX, "epic248-body.md"), "utf8");
  const next = tickBody(body, ["Falconiere", "comemory"], "Falconiere/comemory#257");
  expect(changedLines(body, next).length).toBe(1);
});

test.concurrent("bare ref: acceptance criteria line is not ticked", () => {
  const body = readFileSync(join(FIX, "epic203-body.md"), "utf8");
  expect(body).toContain("- [ ] #205 records");
  expect(tickBody(body, ["Falconiere", "toolu"], "Falconiere/toolu#205", TITLE)).toBe(body);
});

test.concurrent("bare ref: tracking line with title or alone is ticked", () => {
  const body = readFileSync(join(FIX, "epic203-body.md"), "utf8");
  const extended = body + `\n- [ ] #205 ${TITLE}\n- [ ] #205\n`;
  const next = tickBody(extended, ["Falconiere", "toolu"], "Falconiere/toolu#205", TITLE);
  expect(next).toContain(`- [x] #205 ${TITLE}`);
  expect(next.endsWith("- [x] #205\n")).toBe(true);
  expect(next).toContain("- [ ] #205 records");
});

test.concurrent("bare ref: form ignored for other repo", () => {
  const line = `- [ ] #205 ${TITLE}\n`;
  expect(tickBody(line, ["Falconiere", "comemory"], "Falconiere/toolu#205", TITLE)).toBe(line);
});

test.concurrent("protection messages allow admin retry", () => {
  for (const msg of [
    "X Pull request Falconiere/comemory#1 is not mergeable: the base branch policy prohibits the merge.",
    "To use administrator privileges to immediately merge the pull request, add the `--admin` flag.",
  ]) {
    expect(PROTECTION.test(msg)).toBe(true);
  }
});

test.concurrent("protection: other failures do not allow admin retry", () => {
  for (const msg of [
    "Pull request is not mergeable: merge conflict",
    "Head branch was modified. Review and try the merge again.",
  ]) {
    expect(PROTECTION.test(msg)).toBe(false);
  }
});

test.concurrent("auto-merge is pinned to the verified head and deletes the branch", () => {
  expect(autoMergeArgs("o", "r", 12, "abc123", "squash")).toEqual([
    "gh",
    "pr",
    "merge",
    "12",
    "-R",
    "o/r",
    "--auto",
    "--squash",
    "--delete-branch",
    "--match-head-commit",
    "abc123",
  ]);
});

test.concurrent("auto-merge arms only on wait and disarms before any worker push", () => {
  expect(autoMergeAction("wait", false, true)).toBe("arm");
  expect(autoMergeAction("wait", false, false)).toBeNull();
  expect(autoMergeAction("wait", true, true)).toBeNull();
  expect(autoMergeAction("merge", false, true)).toBeNull();
  expect(autoMergeAction("rebase", true, false)).toBe("disarm");
  expect(autoMergeAction("fix", true, true)).toBe("disarm");
  expect(autoMergeAction("fix", false, true)).toBeNull();
});

test.concurrent("recorded auto-merge state reflects this run's arm/disarm outcome", () => {
  // Disarmed this run: off, even though the assessment saw it armed.
  expect(autoMergeState({ auto_merge_armed: true, auto_merge_disarmed: true })).toBe("off");
  expect(autoMergeState({ auto_merge_armed: false, auto_merge: true })).toBe("armed");
  // Arming was tried and the repo refused it.
  expect(autoMergeState({ auto_merge_armed: false, auto_merge: false })).toBe("unavailable");
  expect(autoMergeState({ auto_merge_armed: true })).toBe("armed");
  expect(autoMergeState({ auto_merge_armed: false })).toBe("off");
});
