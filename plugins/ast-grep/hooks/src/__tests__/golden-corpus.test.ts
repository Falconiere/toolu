/**
 * The #258 PreToolUse corpus fixtures search-nudge decides (#268), which left
 * `pre-tools-parity.test.ts` with the bash module: each, with ast-grep's
 * bundled modules registered, gives what the bash module gave through the same
 * pre-tools bundle at the golden's base commit, and the decision class the
 * corpus expects. A named deviation (#283 item 10) must instead show the
 * TypeScript module's answer.
 */
import { expect, test } from "bun:test";
import type { Deviation } from "./cases-types.ts";
import { CASE_TIMEOUT_MS, expectDeviation } from "./golden-expect.ts";
import { CORPUS, caseKey, runCorpus } from "./golden-harness.ts";
import { readGolden } from "./golden-io.ts";

const HOSTS = ["claude", "codex"] as const;
const golden = readGolden().corpus;

/** `grep -r "impl Foo" src`: bash's text match needed whitespace before `impl` (#283 item 10d). */
const CORPUS_DEVIATIONS: Readonly<Record<string, Deviation>> = {
  "ast-grep registry: grep in Bash nudge": {
    contains: "STOP: grep/rg for structural code search.",
    excludes: "grep/rg in Bash detected.",
  },
};

test("every deviation names a fixture", () => {
  const names = new Set(CORPUS.map((f) => f.name));
  for (const name of Object.keys(CORPUS_DEVIATIONS)) expect(names.has(name)).toBe(true);
});

test("the golden holds exactly the ast-grep corpus fixtures", () => {
  const keys = CORPUS.flatMap((f) => HOSTS.map((host) => caseKey(f.name, host)));
  expect(keys.length).toBeGreaterThan(0);
  expect(Object.keys(golden).toSorted()).toEqual(keys.toSorted());
});

for (const f of CORPUS) {
  for (const host of HOSTS) {
    const key = caseKey(f.name, host);
    test.concurrent(
      key,
      async () => {
        const expected = golden[key];
        if (expected === undefined) throw new Error(`no golden capture for ${key}`);
        const actual = await runCorpus(f, host, { kind: "bundle" });
        const deviation = CORPUS_DEVIATIONS[f.name];
        if (deviation === undefined) expect(actual).toEqual(expected);
        else {
          expectDeviation(deviation, actual.stdout, expected.stdout);
          expect({ stderr: actual.stderr, exitCode: actual.exitCode }).toEqual({
            stderr: expected.stderr,
            exitCode: expected.exitCode,
          });
        }
        const want = f.expect[host] ?? f.expect.claude;
        expect(want).toBe("advisory");
        expect(JSON.parse(actual.stdout)).toHaveProperty("hookSpecificOutput.additionalContext");
      },
      CASE_TIMEOUT_MS,
    );
  }
}
