/** Merge-gate pure logic on real GitHub payloads. */

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { PROTECTION, checkBuckets, tickBody } from "../merge-gate.ts";

const FIX = join(import.meta.dir, "..", "fixtures");

describe("CheckBucketsTest", () => {
  test("all green rollup", () => {
    const rollup = JSON.parse(readFileSync(join(FIX, "pr290-rollup.json"), "utf8")) as {
      name: string;
    }[];
    const buckets = checkBuckets(rollup);
    expect(buckets.pass?.length).toBe(rollup.length);
    expect(buckets.fail).toEqual([]);
    expect(buckets.pending).toEqual([]);
  });

  test("failed and running runs are not green", () => {
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

  test("status context states", () => {
    expect(
      checkBuckets([
        { __typename: "StatusContext", context: "ci/a", state: "SUCCESS" },
        { __typename: "StatusContext", context: "ci/b", state: "PENDING" },
        { __typename: "StatusContext", context: "ci/c", state: "ERROR" },
      ]),
    ).toEqual({ pass: ["ci/a"], pending: ["ci/b"], fail: ["ci/c"] });
  });
});

describe("TickBodyTest", () => {
  const body = readFileSync(join(FIX, "epic248-body.md"), "utf8");

  function changedLines(next: string): [string, string][] {
    return body
      .split("\n")
      .map((a, i) => [a, next.split("\n")[i] ?? ""] as [string, string])
      .filter(([a, b]) => a !== b);
  }

  test("ticks exactly the issue line", () => {
    const next = tickBody(body, ["Falconiere", "comemory"], "Falconiere/comemory#255");
    const changed = changedLines(next);
    expect(changed.length).toBe(1);
    const line = changed[0];
    if (!line) throw new Error("no change");
    expect(line[1].startsWith("- [x] https://github.com/Falconiere/comemory/issues/255 ")).toBe(
      true,
    );
  });

  test("cross-repo issue line", () => {
    const next = tickBody(body, ["Falconiere", "comemory"], "CodaSignal/comemory.io#183");
    expect(changedLines(next).length).toBe(1);
  });

  test("prefix number does not match", () => {
    expect(tickBody(body, ["Falconiere", "comemory"], "Falconiere/comemory#25")).toBe(body);
  });

  test("table mentions are untouched", () => {
    const next = tickBody(body, ["Falconiere", "comemory"], "Falconiere/comemory#257");
    expect(changedLines(next).length).toBe(1);
  });
});

describe("BareRefTest", () => {
  const TITLE = "Define shared TS core contracts and verify OpenCode capabilities";
  const body = readFileSync(join(FIX, "epic203-body.md"), "utf8");

  test("acceptance criteria line is not ticked", () => {
    expect(body).toContain("- [ ] #205 records");
    expect(tickBody(body, ["Falconiere", "toolu"], "Falconiere/toolu#205", TITLE)).toBe(body);
  });

  test("bare tracking line with title or alone is ticked", () => {
    const extended = body + `\n- [ ] #205 ${TITLE}\n- [ ] #205\n`;
    const next = tickBody(extended, ["Falconiere", "toolu"], "Falconiere/toolu#205", TITLE);
    expect(next).toContain(`- [x] #205 ${TITLE}`);
    expect(next.endsWith("- [x] #205\n")).toBe(true);
    expect(next).toContain("- [ ] #205 records");
  });

  test("bare form ignored for other repo", () => {
    const line = `- [ ] #205 ${TITLE}\n`;
    expect(tickBody(line, ["Falconiere", "comemory"], "Falconiere/toolu#205", TITLE)).toBe(line);
  });
});

describe("ProtectionTest", () => {
  test("protection messages allow admin retry", () => {
    for (const msg of [
      "X Pull request Falconiere/comemory#1 is not mergeable: the base branch policy prohibits the merge.",
      "To use administrator privileges to immediately merge the pull request, add the `--admin` flag.",
    ]) {
      expect(PROTECTION.test(msg)).toBe(true);
    }
  });

  test("other failures do not", () => {
    for (const msg of [
      "Pull request is not mergeable: merge conflict",
      "Head branch was modified. Review and try the merge again.",
    ]) {
      expect(PROTECTION.test(msg)).toBe(false);
    }
  });
});
