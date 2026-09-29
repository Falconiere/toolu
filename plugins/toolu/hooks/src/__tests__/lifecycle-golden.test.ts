/**
 * Byte parity with the bash hooks (#263): each case runs the committed bundle
 * in the same sandbox the bash script ran in and must print what bash printed.
 * SessionStart's `systemMessage` also carries the #250 runtime line after the
 * bash title, so only its first line is compared.
 */
import { describe, expect, test } from "bun:test";
import { z } from "zod";
import { bundleArgv, runCase, type Captured, type LifecycleCase } from "./lifecycle-cases.ts";
import { readGolden } from "./lifecycle-golden.ts";
import { SESSION_START_CASES } from "./session-start-cases.ts";
import { USER_PROMPT_SUBMIT_CASES } from "./user-prompt-submit-cases.ts";

const golden = readGolden().cases;

/** Each case spawns real git and Bun processes; 5 s is too tight on a loaded machine. */
const CASE_TIMEOUT_MS = 30_000;

function expected(c: LifecycleCase): Captured {
  const found = golden[c.name];
  if (found === undefined) throw new Error(`no golden capture for ${c.name}`);
  return found;
}

const SessionOutputSchema = z.object({
  hookSpecificOutput: z.object({ additionalContext: z.string() }).optional(),
  systemMessage: z.string().optional(),
});

/** Split a SessionStart output into what bash owned: context and title. */
function sessionParts(stdout: string): { context: string | undefined; title: string } {
  const out = SessionOutputSchema.parse(JSON.parse(stdout));
  return {
    context: out.hookSpecificOutput?.additionalContext,
    title: (out.systemMessage ?? "").split("\n")[0] ?? "",
  };
}

describe("session-start", () => {
  test.concurrent.each(SESSION_START_CASES.map((c) => [c.name, c] as const))(
    "%s",
    async (_n, c) => {
      const want = expected(c);
      const got = await runCase(c, bundleArgv);
      expect(got.exitCode).toBe(want.exitCode);
      expect(got.stderr).toBe(want.stderr);
      expect(sessionParts(got.stdout)).toEqual(sessionParts(want.stdout));
      expect(got.stdout.endsWith("}\n")).toBe(true);
    },
    CASE_TIMEOUT_MS,
  );
});

describe("user-prompt-submit", () => {
  test.concurrent.each(USER_PROMPT_SUBMIT_CASES.map((c) => [c.name, c] as const))(
    "%s",
    async (_n, c) => {
      const want = expected(c);
      const got = await runCase(c, bundleArgv);
      expect(got).toEqual(want);
    },
    CASE_TIMEOUT_MS,
  );
});
