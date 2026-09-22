import { expect, test } from "bun:test";
import { runConformanceMatrix, runProtectedFilesConformance } from "../run.ts";

test("runProtectedFilesConformance passes on real protected-files bridge", async () => {
  const result = await runProtectedFilesConformance();
  expect(result.pass).toBe(true);
});

test("runConformanceMatrix passes all fixture suites (live lane skipped by default)", async () => {
  const { pass, results } = await runConformanceMatrix();
  const live = results.find((r) => r.id === "live-opencode");
  expect(live?.outcome.status).toBe("skip");
  if (live?.outcome.status === "skip") {
    expect(live.outcome.message).toContain("TOOLU_LIVE_OPENCODE");
  }
  expect(pass).toBe(true);
  for (const { id, outcome } of results) {
    if (id === "live-opencode") {
      continue;
    }
    expect(outcome.status).toBe("pass");
  }
});
