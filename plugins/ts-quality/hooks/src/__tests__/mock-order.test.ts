/**
 * The no-mocks excerpt (#265) shows the first five hits in source order. Bash
 * showed ast-grep's multi-rule output unsorted, so which five appeared varied
 * run to run; the golden can only compare a five-hit file as a set.
 */
import { expect, test } from "bun:test";
import { TS_PROJECT } from "./cases-types.ts";
import { runCase } from "./golden-harness.ts";

test("seven mock calls show lines 1-5, in order", async () => {
  const body =
    'vi.fn();\nsinon.spy();\njest.fn();\nvi.mock("a");\njest.mock("b");\nsinon.stub();\nvi.fn();\n';
  const [step] = await runCase(
    {
      name: "mock order",
      project: TS_PROJECT,
      steps: [{ write: { "src/__tests__/foo.test.ts": body }, file: "src/__tests__/foo.test.ts" }],
      expect: "advisory",
    },
    "claude",
    { kind: "bundle" },
  );
  const lines = [...(step?.stdout ?? "").matchAll(/foo\.test\.ts:(\d+):/g)].map((m) => m[1]);
  expect(lines).toEqual(["1", "2", "3", "4", "5"]);
}, 60_000);
