import { expect, test } from "bun:test";
import { runProtectedFilesConformance } from "../run.ts";

test("runProtectedFilesConformance passes on real protected-files bridge", async () => {
  const result = await runProtectedFilesConformance();
  expect(result.pass).toBe(true);
});
