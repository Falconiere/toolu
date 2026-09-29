/**
 * AC-1 and AC-2 (#259): every PostToolUse fixture, rendered as Claude Code and
 * as Codex deliver it, gives byte-identical stdout, the same exit code and the
 * same project state (gate file, waivers, telemetry) from `bash
 * post-tools/mod.sh` and from the committed bundle behind its launcher, both
 * started from the same sandbox. The language-quality modules registered by
 * their real `register.sh` run through the bundle on bash.
 */
import { expect, test } from "bun:test";
import { isJsonObject } from "@toolu/core/config";
import { runPostBundle, runPostModSh } from "@toolu/conformance/harness/posttool";
import {
  POSTTOOL_CORPUS,
  postStdin,
  preparePost,
  type PostOutcome,
} from "@toolu/conformance/harness/posttool-corpus";
import { fromSameState, pretoolEnv, type PretoolHost } from "@toolu/conformance/harness/pretool";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

const HOSTS: PretoolHost[] = ["claude", "codex"];

/**
 * The gate file with its top-level `violations` in entry-key order. Both
 * implementations order that aggregate by second-resolution `updatedAt`, then
 * key, so two entries written across a second boundary swap on either side.
 * Each side's aggregate must still be exactly its own entries' violations.
 */
function canonicalGate(text: string): string {
  const doc: unknown = JSON.parse(text);
  if (!isJsonObject(doc) || !isJsonObject(doc.entries)) return text;
  const parts = Object.keys(doc.entries)
    .toSorted()
    .map((key) => {
      const entry = isJsonObject(doc.entries) ? doc.entries[key] : undefined;
      return isJsonObject(entry) && typeof entry.violations === "string" ? entry.violations : "";
    });
  const aggregate = typeof doc.violations === "string" ? doc.violations : "";
  expect(aggregate.length).toBe(parts.join("").length);
  for (const part of parts) expect(aggregate).toContain(part);
  return JSON.stringify({ ...doc, violations: parts.join("") }, null, 2);
}

function canonical(state: Record<string, string>): Record<string, string> {
  return Object.fromEntries(
    Object.entries(state).map(([path, text]) => [
      path,
      path.endsWith("quality-gate-status.json") ? canonicalGate(text) : text,
    ]),
  );
}

function outcomeOf(stdout: string, exitCode: number): PostOutcome {
  if (exitCode === 2) return "exit2";
  if (stdout.trim() === "") return "silent";
  const out: unknown = JSON.parse(stdout);
  return isJsonObject(out) && out.decision === "block" ? "block" : "advisory";
}

for (const c of POSTTOOL_CORPUS) {
  for (const host of HOSTS) {
    test.concurrent(`${c.name} [${host}]`, async () => {
      using sb = createSandbox({ git: true });
      await preparePost(sb, host, c);
      const call = { cwd: sb.project, env: pretoolEnv(sb, host), stdin: postStdin(sb, host, c) };
      const [bash, bundle] = await fromSameState(
        sb,
        () => runPostModSh(sb, call),
        () => runPostBundle(sb, call),
      );
      const seen = (r: typeof bash) => ({
        stdout: r.stdout,
        exitCode: r.exitCode,
        state: canonical(r.state),
      });
      expect(seen(bundle)).toEqual(seen(bash));
      expect(outcomeOf(bundle.stdout, bundle.exitCode)).toBe(c.expect);
    });
  }
}

test("the corpus reaches every decision class", () => {
  const classes = new Set(POSTTOOL_CORPUS.map((c) => c.expect));
  expect([...classes].toSorted()).toEqual(["advisory", "block", "exit2", "silent"]);
});
