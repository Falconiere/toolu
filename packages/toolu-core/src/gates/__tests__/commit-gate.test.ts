/**
 * commit-gate (#261): the Conventional Commits type of a subject, on the
 * message forms #283 item 5 lists and on the bats suite's escaped-quote
 * regressions. The whole gate runs through the bundle in
 * `plugins/toolu/hooks/src/__tests__/pre-tool-modules-b.test.ts`.
 */
import { expect, test } from "bun:test";
import { z } from "zod";
import { commitMessages, gitInvocation } from "../../shell/shell-git.ts";
import { analyzeShell } from "../../shell/shell-parse.ts";
import { readFixture } from "../../shell/__tests__/parity-helpers.ts";
import { commitPrefix } from "../commit-gate.ts";

test.each([
  ["feat: add widget", "feat"],
  ['fix: handle "quoted" text', "fix"],
  ['wibble("ui"): add stuff', "wibble"],
  ["feat(scope): x", "feat"],
  ["wibble: x\n\nbody", "wibble"],
])("%p has type %p", (message, type) => {
  expect(commitPrefix(message)).toBe(type);
});

test.each([["wip"], ["wibble!: x"], ["Feat: x"], ["\nfeat: x"], [""], ["feat : x"]])(
  "%p has no type",
  (message) => {
    expect(commitPrefix(message)).toBeUndefined();
  },
);

const MessageCase = z.object({ kind: z.string(), command: z.string() }).passthrough();

test("every #283 item 5 commit form yields the wibble subject", () => {
  const { cases } = z.object({ cases: z.array(MessageCase) }).parse(readFixture("issue-283.json"));
  const forms = cases.filter((c) => c.kind === "commit_messages");
  expect(forms.length).toBeGreaterThanOrEqual(4);
  for (const c of forms) {
    const [command] = analyzeShell(c.command).commands.filter(
      (cmd) => gitInvocation(cmd) !== undefined,
    );
    const git = command === undefined ? undefined : gitInvocation(command);
    const subject = git === undefined ? undefined : commitMessages(git)[0];
    expect(typeof subject === "string" ? commitPrefix(subject) : undefined).toBe("wibble");
  }
});
