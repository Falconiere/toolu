/**
 * AC-1 (#258): every PreToolUse fixture, rendered as Claude Code and as Codex
 * deliver it, gives byte-identical stdout and the same exit code from
 * `bash pre-tools/mod.sh` and from the committed bundle behind its launcher,
 * with every module still on bash fallback. A fixture decided by a module
 * whose bash script is deleted (#261) is compared with what `mod.sh` printed
 * for it before the deletion, as parsed JSON: the native encoder prints
 * compact JSON where jq printed it pretty. Fixtures protected-files,
 * mcp-blocker and code-edit-rules decide (#260) are replayed against their
 * own capture in `pre-tool-modules-a-golden.test.ts`.
 */
import { expect, test } from "bun:test";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import {
  fromSameState,
  pretoolEnv,
  runBundle,
  runModSh,
  type PretoolHost,
} from "@toolu/conformance/harness/pretool";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { PRETOOL_CORPUS, prepare, type Outcome } from "@toolu/conformance/harness/pretool-corpus";
import { comparable } from "./pre-tool-modules-b-cases.ts";
import { corpusKey, decidedByModulesB, readGolden } from "./pre-tool-modules-b-golden.ts";
import { readGolden as readGoldenA } from "./pre-tool-modules-a-golden.ts";

const HOSTS: PretoolHost[] = ["claude", "codex"];

const golden = readGolden().corpus;
const PORTED_A = readGoldenA().corpus;

function outcomeOf(stdout: string, exitCode: number): Outcome {
  if (exitCode === 2) return "exit2";
  if (stdout.trim() === "") return "silent";
  const out: unknown = JSON.parse(stdout);
  const text = JSON.stringify(out);
  if (text.includes('"permissionDecision":"deny"')) return "deny";
  if (text.includes('"permissionDecision":"ask"')) return "ask";
  return "advisory";
}

for (const fixture of PRETOOL_CORPUS) {
  for (const host of HOSTS.filter((h) => PORTED_A[corpusKey(fixture.name, h)] === undefined)) {
    test.concurrent(`${fixture.name} [${host}]`, async () => {
      using sb = createSandbox({ git: true });
      const extra = await prepare(sb, host, fixture);
      const stdin =
        fixture.stdin ?? JSON.stringify(toStdin(host, fixture.fixture(sb), { cwd: sb.project }));
      const call = { cwd: sb.project, env: pretoolEnv(sb, host, extra), stdin };
      if (decidedByModulesB(fixture.name)) {
        const want = golden[corpusKey(fixture.name, host)];
        if (want === undefined) throw new Error(`no golden capture for ${fixture.name} [${host}]`);
        const got = await runBundle(call);
        const norm = (text: string) => text.split(sb.root).join("$ROOT");
        const captured = {
          stdout: norm(got.stdout),
          stderr: norm(got.stderr),
          exitCode: got.exitCode,
        };
        expect(comparable(captured)).toEqual(comparable(want));
        expect(outcomeOf(got.stdout, got.exitCode)).toBe(
          fixture.expect[host] ?? fixture.expect.claude,
        );
        return;
      }
      const [bash, bundle] = await fromSameState(
        sb,
        () => runModSh(call),
        () => runBundle(call),
      );
      expect({ stdout: bundle.stdout, exitCode: bundle.exitCode }).toEqual({
        stdout: bash.stdout,
        exitCode: bash.exitCode,
      });
      expect(outcomeOf(bundle.stdout, bundle.exitCode)).toBe(
        fixture.expect[host] ?? fixture.expect.claude,
      );
    });
  }
}

test("the corpus reaches every decision class", () => {
  const classes = new Set(
    PRETOOL_CORPUS.flatMap((f) => HOSTS.map((host) => f.expect[host] ?? f.expect.claude)),
  );
  expect([...classes].toSorted()).toEqual(["advisory", "ask", "deny", "exit2", "silent"]);
});
