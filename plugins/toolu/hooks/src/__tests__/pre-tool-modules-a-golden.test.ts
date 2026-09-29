/**
 * AC-1 (#260): the #258 PreToolUse corpus cases that protected-files,
 * mcp-blocker and code-edit-rules decide, as Claude Code and Codex deliver
 * them, give the bundle the decision, exit code and stderr bash gave at the
 * base commit. The other corpus cases stay byte-for-byte against live
 * `mod.sh` in `pre-tools-parity.test.ts`.
 */
import { expect, test } from "bun:test";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import { pretoolEnv, runBundle } from "@toolu/conformance/harness/pretool";
import { PRETOOL_CORPUS, prepare } from "@toolu/conformance/harness/pretool-corpus";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { corpusKey, readGolden } from "./pre-tool-modules-a-golden.ts";
import { comparable } from "./pre-tool-modules-a-parity.ts";

const golden = readGolden().corpus;
const CASES = PRETOOL_CORPUS.flatMap((fixture) =>
  (["claude", "codex"] as const)
    .filter((host) => golden[corpusKey(fixture.name, host)] !== undefined)
    .map((host) => ({ fixture, host })),
);

test("the golden corpus is the 22 rows these modules decide", () => {
  expect(CASES.length).toBe(Object.keys(golden).length);
  expect(CASES.length).toBe(22);
});

for (const { fixture, host } of CASES) {
  test.concurrent(`${fixture.name} [${host}]`, async () => {
    using sb = createSandbox({ git: true });
    const extra = await prepare(sb, host, fixture);
    const stdin =
      fixture.stdin ?? JSON.stringify(toStdin(host, fixture.fixture(sb), { cwd: sb.project }));
    const out = await runBundle({ cwd: sb.project, env: pretoolEnv(sb, host, extra), stdin });
    const norm = (text: string) => text.split(sb.root).join("$ROOT");
    const got = { stdout: norm(out.stdout), stderr: norm(out.stderr), exitCode: out.exitCode };
    expect(comparable(got)).toEqual(comparable(golden[corpusKey(fixture.name, host)] ?? got));
  }, 30_000);
}
