/** Every committed post-tool corpus record reaches the real bundle on both hosts. */
import { expect, test } from "bun:test";
import { runPostBundle } from "../posttool.ts";
import { POSTTOOL_CORPUS, postStdin, preparePost, type PostOutcome } from "../posttool-corpus.ts";
import { pretoolEnv } from "../pretool.ts";
import { createSandbox } from "../sandbox.ts";

const HOSTS = ["claude", "codex"] as const;

function outcome(stdout: string, exitCode: number): PostOutcome {
  if (exitCode === 2) return "exit2";
  if (stdout.trim() === "") return "silent";
  const value: unknown = JSON.parse(stdout);
  if (
    value !== null &&
    typeof value === "object" &&
    "decision" in value &&
    value.decision === "block"
  ) {
    return "block";
  }
  return "advisory";
}

for (const c of POSTTOOL_CORPUS) {
  for (const host of HOSTS) {
    test.concurrent(`${c.name} [${host}]`, async () => {
      using sb = createSandbox({ git: true });
      preparePost(sb, host, c);
      const stdin = postStdin(sb, host, c);
      const result = await runPostBundle(sb, {
        cwd: sb.project,
        env: pretoolEnv(sb, host),
        stdin,
      });
      expect(outcome(result.stdout, result.exitCode)).toBe(c.expect);
    }, 30_000);
  }
}
