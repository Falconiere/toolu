/** Graph scheduling tests on the real epic Falconiere/comemory#248 snapshot. */

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { issueKey, parseRef, slugify } from "../common.ts";
import { classify, computeLevels, downstreamCounts, type GraphIssue } from "../epic-graph.ts";

const FIXTURE = join(import.meta.dir, "..", "fixtures", "epic248-graph.json");

function loadIssues(): Record<string, GraphIssue> {
  const graph = JSON.parse(readFileSync(FIXTURE, "utf8")) as {
    issues: GraphIssue[];
  };
  return Object.fromEntries(graph.issues.map((i) => [i.ref, i]));
}

describe("GraphTest", () => {
  const issues = loadIssues();

  test("waves follow open blockers", () => {
    const [levels, cycle] = computeLevels(issues);
    expect(cycle).toEqual([]);
    expect(levels).toEqual({
      "Falconiere/comemory#254": 1,
      "Falconiere/comemory#255": 1,
      "CodaSignal/comemory.io#183": 1,
      "Falconiere/comemory#256": 2,
      "Falconiere/comemory#257": 2,
      "CodaSignal/comemory.io#184": 2,
      "Falconiere/comemory#258": 3,
      "CodaSignal/comemory.io#185": 3,
      "Falconiere/homebrew-tap#1": 4,
    });
  });

  test("critical path issue unblocks most", () => {
    const counts = downstreamCounts(issues);
    expect(counts["Falconiere/comemory#255"]).toBe(5);
    expect(counts["Falconiere/comemory#254"]).toBe(2);
    expect(counts["CodaSignal/comemory.io#183"]).toBe(2);
    expect(counts["Falconiere/homebrew-tap#1"]).toBe(0);
  });

  test("classify ready blocked done and in_flight", () => {
    const refs = new Set(Object.keys(issues));
    const i255 = issues["Falconiere/comemory#255"];
    const i257 = issues["Falconiere/comemory#257"];
    const i253 = issues["Falconiere/comemory#253"];
    if (!i255 || !i257 || !i253) throw new Error("fixture missing issues");
    expect(classify(i255, refs, {})).toBe("ready");
    expect(classify(i257, refs, {})).toBe("blocked");
    expect(classify(i253, refs, {})).toBe("done");
    expect(classify(i255, refs, { "comemory-255": { stage: "running" } })).toBe("in_flight");
    expect(classify(i255, refs, { "comemory-255": { stage: "merged" } })).toBe("ready");
  });

  test("blocker outside epic is external", () => {
    const base = issues["Falconiere/comemory#254"];
    if (!base) throw new Error("fixture missing #254");
    const issue = {
      ...base,
      blockers: { ...base.blockers, "Falconiere/other#9": "open" },
    };
    expect(classify(issue, new Set(Object.keys(issues)), {})).toBe("external_blocked");
  });

  test("cycle is reported not scheduled", () => {
    const cloned = structuredClone(issues) as Record<string, GraphIssue>;
    const i255 = cloned["Falconiere/comemory#255"];
    if (!i255) throw new Error("fixture missing #255");
    i255.blockers = { ...i255.blockers, "Falconiere/comemory#257": "open" };
    const [levels, cycle] = computeLevels(cloned);
    expect(cycle).toContain("Falconiere/comemory#255");
    expect(cycle).toContain("Falconiere/comemory#257");
    expect(levels).not.toHaveProperty("Falconiere/comemory#255");
  });
});

describe("CommonTest", () => {
  test("parse_ref forms", () => {
    expect(parseRef("https://github.com/Falconiere/comemory/issues/248")).toEqual([
      "Falconiere",
      "comemory",
      248,
    ]);
    expect(parseRef("CodaSignal/comemory.io#183")).toEqual(["CodaSignal", "comemory.io", 183]);
    expect(parseRef("#12", "a/b")).toEqual(["a", "b", 12]);
    expect(() => parseRef("not a ref", "a/b")).toThrow();
  });

  test("issue_key is a valid herdr agent name", () => {
    const name = /^[a-z][a-z0-9_-]{0,31}$/;
    for (const [repo, n] of [
      ["comemory.io", 183],
      ["homebrew-tap", 1],
      ["9lives", 3],
      ["an-extremely-long-repository-name-for-testing", 12345],
      ["Falconiere/comemory", 255],
    ] as const) {
      const key = issueKey(repo, n);
      expect(key).toMatch(name);
      expect(key.endsWith(`-${n}`)).toBe(true);
    }
    expect(issueKey("comemory.io", 183)).toBe("comemory-io-183");
    expect(issueKey("Falconiere/comemory", 255)).toBe("falconiere-comemory-255");
  });

  test("issue_key folds owner so same-name repos do not collide", () => {
    expect(issueKey("orgA/foo", 1)).not.toBe(issueKey("orgB/foo", 1));
    expect(issueKey("orgA/foo", 1)).toMatch(/^[a-z][a-z0-9_-]{0,31}$/);
    expect(issueKey("orgB/foo", 1)).toMatch(/^[a-z][a-z0-9_-]{0,31}$/);
  });

  test("slugify drops epic prefix", () => {
    expect(slugify("[Realtime replication] Drain durable push/pull backlogs safely")).toBe(
      "drain-durable-push-pull-backlogs",
    );
  });
});
