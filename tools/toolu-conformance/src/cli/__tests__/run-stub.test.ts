import { expect, test } from "bun:test";
import { runProtectedFilesConformance } from "../run-stub.ts";

test("run-stub re-exports conformance runner", async () => {
  const result = await runProtectedFilesConformance();
  expect(result.pass).toBe(true);
});
