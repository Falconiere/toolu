/**
 * AC-1 (#258): every PreToolUse fixture, rendered as Claude Code and as Codex
 * deliver it, receives the expected decision from the committed dispatcher or
 * the compiled ast-grep rule after its native registration.
 * Captured Bash output for ported gates is replayed by the A, B and C golden
 * suites; this corpus also exercises registry and dispatcher-only cases.
 */
import { expect, test } from "bun:test";
import {
  implementationTag,
  requiredBuiltTooluBinary,
} from "@toolu/conformance/harness/entry-command";
import { pretoolEnv, runBundle, type PretoolHost } from "@toolu/conformance/harness/pretool";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import {
  PRETOOL_CORPUS,
  prepare,
  pretoolStdin,
  type Outcome,
} from "@toolu/conformance/harness/pretool-corpus";
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

const IMPL = implementationTag("toolu", "pre-tools");

for (const fixture of PRETOOL_CORPUS) {
  for (const host of HOSTS.filter((h) => PORTED_A[corpusKey(fixture.name, h)] === undefined)) {
    test.concurrent(`${fixture.name} [${host}]${IMPL}`, async () => {
      using sb = createSandbox({ git: true });
      const extra = await prepare(sb, host, fixture);
      const stdin = pretoolStdin(sb, host, fixture);
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
      const bundle = fixture.name.startsWith("ast-grep registry:")
        ? await run(
            [requiredBuiltTooluBinary(), "ast-grep", "hook", "pre-tools", "--event", "PreToolUse"],
            call,
          )
        : await runBundle(call);
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
