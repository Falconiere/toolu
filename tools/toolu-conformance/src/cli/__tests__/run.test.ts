import { expect, test } from "bun:test";
import { runConformanceMatrix, runProtectedFilesConformance } from "../run.ts";

test("runProtectedFilesConformance passes on real protected-files bridge", async () => {
  const result = await runProtectedFilesConformance();
  expect(result.pass).toBe(true);
});

test("runConformanceMatrix passes every fixture suite; none skips (#362)", async () => {
  const { pass, results } = await runConformanceMatrix();
  expect(results.map((r) => r.id)).toEqual([
    "protected-files",
    "bootstrap-readiness",
    "surface-drift",
    "spaces-cwd",
  ]);
  expect(results.map((r) => r.outcome)).toEqual(results.map(() => ({ status: "pass" })));
  expect(pass).toBe(true);
});

test("runConformanceMatrix fails closed when one suite fails", async () => {
  const { pass, results } = await runConformanceMatrix([
    { id: "ok", run: () => Promise.resolve({ status: "pass" }) },
    { id: "broken", run: () => Promise.reject(new Error("suite crashed")) },
  ]);
  expect(results[1]?.outcome).toEqual({ status: "fail", message: "suite crashed" });
  expect(pass).toBe(false);
});
