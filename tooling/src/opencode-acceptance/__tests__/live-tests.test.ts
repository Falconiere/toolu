import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { parseJUnit, runLiveFile } from "../live-tests.ts";

// A real `bun test --reporter=junit` run: the parser reads what Bun writes, not a hand-made sample (#362 AC-4).

const SAMPLE = `import { expect, test } from "bun:test";
test("host check passes", () => { expect(1).toBe(1); });
test.skipIf(process.env.SAMPLE_EXTERNAL !== "1")("external <service> & \\"quoted\\" check", () => {
  expect(1).toBe(1);
});
test("broken check fails", () => { expect(1).toBe(2); });
`;

test.concurrent("a real JUnit report yields passed, skipped and failed cases by name", async () => {
  using sb = createSandbox();
  sb.write("sample.test.ts", SAMPLE);
  const { cases, exitCode } = await runLiveFile(sb.path("sample.test.ts"), {});
  expect(exitCode).toBe(1);
  expect(cases.map(({ name, status }) => ({ name, status }))).toEqual([
    { name: "host check passes", status: "passed" },
    { name: 'external <service> & "quoted" check', status: "skipped" },
    { name: "broken check fails", status: "failed" },
  ]);
});

test.concurrent("the external flag un-skips only its own test", async () => {
  using sb = createSandbox();
  sb.write("sample.test.ts", SAMPLE.replace('test("broken check fails"', 'test.skip("broken"'));
  const { cases, exitCode } = await runLiveFile(sb.path("sample.test.ts"), {
    SAMPLE_EXTERNAL: "1",
  });
  expect(exitCode).toBe(0);
  expect(cases.map((item) => item.status)).toEqual(["passed", "passed", "skipped"]);
});

test.concurrent("a file that does not exist yields no cases and a failing exit", async () => {
  using sb = createSandbox();
  const { cases, exitCode } = await runLiveFile(sb.path("missing.test.ts"), {});
  expect(cases).toEqual([]);
  expect(exitCode).not.toBe(0);
});

test.concurrent("parseJUnit reads self-closing and bodied test cases", () => {
  const xml = [
    '<testcase name="a" classname="" time="0.5" />',
    '<testcase name="b" classname="x" time="1"><skipped /></testcase>',
    '<testcase name="c" time="2"><failure message="m" /></testcase>',
  ].join("\n");
  expect(parseJUnit(xml)).toEqual([
    { name: "a", status: "passed", seconds: 0.5 },
    { name: "b", status: "skipped", seconds: 1 },
    { name: "c", status: "failed", seconds: 2 },
  ]);
  expect(parseJUnit("")).toEqual([]);
});
