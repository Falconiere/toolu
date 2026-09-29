/** Epic summary rendered from the #248 snapshot. */

import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { summaryTable } from "../epic-close.ts";

test.concurrent("summary lists every sub-issue with its merged PR and admin merges", () => {
  const graph = JSON.parse(
    readFileSync(join(import.meta.dir, "..", "fixtures", "epic248-graph.json"), "utf8"),
  ) as Parameters<typeof summaryTable>[0];
  const first = graph.issues[0];
  if (!first) throw new Error("fixture has no issues");
  first.prs = [
    { number: 7, state: "MERGED", url: "https://github.com/x/y/pull/7", headRefName: "b" },
  ];
  const text = summaryTable(graph, { [first.key]: { last_gate: { admin_used: true } } });
  const lines = text.split("\n");
  expect(lines.slice(2, 4)).toEqual(["| Sub-issue | Merged PR |", "|---|---|"]);
  expect(lines).toHaveLength(4 + graph.issues.length);
  expect(text).toContain(`| ${first.ref} — `);
  expect(text).toContain("https://github.com/x/y/pull/7 (admin merge)");
  expect(text).toContain("closed without a merged PR");
});
