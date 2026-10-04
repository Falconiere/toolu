import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  liveTestChecks,
  parseJUnit,
  redact,
  runExternal,
  runLiveFile,
  type LiveTestFile,
} from "../live-tests.ts";

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

const LIVE_SAMPLE = `import { expect, test } from "bun:test";
test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")("host check", () => { expect(1).toBe(1); });
test.skipIf(process.env.SAMPLE_LIVE !== "1")("external check", () => {
  console.error("echo " + process.env.SAMPLE_KEY);
  expect(process.env.SAMPLE_FAIL).toBeUndefined();
});
`;
const EXTERNAL = {
  id: "external.sample",
  name: "external check",
  flag: "SAMPLE_LIVE",
  service: "sample.test",
};
const CTX = { bin: "", cacheRoot: "", tarball: "" };

function liveFile(path: string): LiveTestFile {
  return {
    id: "live.sample",
    file: path,
    plugins: ["toolu"],
    service: "fixture",
    external: [EXTERNAL],
  };
}

test.concurrent("a live file passes only with its host test passed and its external test skipped", async () => {
  using sb = createSandbox();
  sb.write("sample.live.test.ts", LIVE_SAMPLE);
  const [check] = liveTestChecks([liveFile(sb.path("sample.live.test.ts"))]);
  const outcome = await check?.run(CTX);
  expect(outcome?.pass).toBe(true);
  expect(outcome?.observed.cases).toBe("host check: passed; external check: skipped");
  expect(check?.evidence).toEqual({
    execution: "actual-host",
    model: "scripted-loopback",
    service: "fixture",
  });
});

test.concurrent("a skipped host test fails the live check instead of passing quietly", async () => {
  using sb = createSandbox();
  sb.write("sample.live.test.ts", LIVE_SAMPLE.replace("TOOLU_LIVE_OPENCODE", "NEVER_SET_FLAG"));
  const [check] = liveTestChecks([liveFile(sb.path("sample.live.test.ts"))]);
  const outcome = await check?.run(CTX);
  expect(outcome?.pass).toBe(false);
  expect(outcome?.observed.cases).toBe("host check: skipped; external check: skipped");
});

test.concurrent("an external test reports available, unavailable or not-configured, never a key", async () => {
  using sb = createSandbox();
  sb.write("sample.live.test.ts", LIVE_SAMPLE);
  const entry = liveFile(sb.path("sample.live.test.ts"));
  const keyed = { ...EXTERNAL, requires: "SAMPLE_KEY" };
  expect(await runExternal(entry, keyed, {})).toEqual({
    id: "external.sample",
    service: "sample.test",
    execution: "in-process",
    status: "not-configured",
    detail: "SAMPLE_KEY is not set",
  });
  const available = await runExternal(entry, keyed, { SAMPLE_KEY: "sk-sample-362" });
  expect(available.status).toBe("available");
  expect(available.detail).toMatch(/^passed in /);
  sb.write("broken.live.test.ts", LIVE_SAMPLE.replace('"external check"', '"renamed check"'));
  const missing = await runExternal(liveFile(sb.path("broken.live.test.ts")), keyed, {
    SAMPLE_KEY: "sk-sample-362",
  });
  expect(missing.status).toBe("unavailable");
  expect(missing.detail).toStartWith("not run: ");
  expect(missing.detail).not.toContain("sk-sample-362");
});

test.concurrent("redact replaces every secret value and ignores empty ones", () => {
  expect(redact("key sk-1 and sk-1 and sk-2", ["sk-1", "sk-2", ""])).toBe(
    "key *** and *** and ***",
  );
});
