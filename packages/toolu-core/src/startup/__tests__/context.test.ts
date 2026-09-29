/** SessionStart context payloads, byte-compared with the jq the bash hooks used (#269). */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { MAX_CONTEXT_CHARS, renderHookOutput, sessionContext } from "../context.ts";

const NO_JQ = spawnSync("jq", ["--version"]).status !== 0;

/** What `jq -n[c] --arg event … --arg context …` prints for the same payload. */
function jqRender(event: string, context: string, pretty: boolean): string {
  const filter = "{hookSpecificOutput: {hookEventName: $event, additionalContext: $context}}";
  const flags = pretty ? ["-n"] : ["-nc"];
  const args = [...flags, "--arg", "event", event, "--arg", "context", context, filter];
  const res = spawnSync("jq", args, { encoding: "utf8" });
  if (res.status !== 0) throw new Error(`jq failed: ${res.stderr}`);
  return res.stdout;
}

test.concurrent("empty text yields no payload", () => {
  expect(sessionContext("SessionStart", "")).toBeUndefined();
});

test.concurrent("text within the bound is carried verbatim", () => {
  expect(sessionContext("SessionStart", 'say "hi"\n\ttab')).toEqual({
    hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: 'say "hi"\n\ttab' },
  });
});

test.concurrent("text past the bound is cut to MAX_CONTEXT_CHARS", () => {
  const text = "x".repeat(MAX_CONTEXT_CHARS + 50);
  const out = sessionContext("SessionStart", text);
  expect(out?.hookSpecificOutput.additionalContext).toBe("x".repeat(MAX_CONTEXT_CHARS));
});

test.concurrent("the cut never splits a surrogate pair", () => {
  const text = `${"x".repeat(MAX_CONTEXT_CHARS - 1)}😀tail`;
  const out = sessionContext("SessionStart", text);
  expect(out?.hookSpecificOutput.additionalContext).toBe("x".repeat(MAX_CONTEXT_CHARS - 1));
});

test.concurrent.skipIf(NO_JQ)("pretty and compact output match jq byte for byte", () => {
  const text = 'paths "q" \\ /x\nline\u007f é 😀 \u0001';
  const payload = sessionContext("SessionStart", text);
  expect(renderHookOutput(payload, true)).toBe(jqRender("SessionStart", text, true));
  expect(renderHookOutput(payload, false)).toBe(jqRender("SessionStart", text, false));
});
