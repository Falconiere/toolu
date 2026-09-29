import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { HostOutputError, readHostOutcome, readOpencodeOutcome } from "../hosts.ts";
import { run, type RunResult } from "../spawn.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
const GATE_MODE = join(ROOT, "plugins/toolu/hooks/lib/gate-mode.sh");

/** The real PreToolUse JSON toolu's bash gates emit for a mode. */
function gateEmit(mode: "block" | "ask" | "advise" | "off"): Promise<RunResult> {
  return run([
    "bash",
    "-c",
    '. "$1" && toolu_gate_emit "$2" "why it matters"',
    "_",
    GATE_MODE,
    mode,
  ]);
}

function result(stdout: string, exitCode = 0, stderr = ""): RunResult {
  return { exitCode, stdout, stderr, durationMs: 1, timedOut: false };
}

test.concurrent("claude: real gate output maps block/ask/advise/off to deny/ask/allow", async () => {
  expect(readHostOutcome("claude", "PreToolUse", await gateEmit("block"))).toEqual({
    effect: "deny",
    reason: "why it matters",
  });
  expect(readHostOutcome("claude", "PreToolUse", await gateEmit("ask"))).toEqual({
    effect: "ask",
    reason: "why it matters",
  });
  expect(readHostOutcome("claude", "PreToolUse", await gateEmit("advise"))).toEqual({
    effect: "allow",
    context: "why it matters",
  });
  expect(readHostOutcome("claude", "PreToolUse", await gateEmit("off"))).toEqual({
    effect: "allow",
  });
});

test.concurrent("codex: accepts deny and rejects ask, which Codex cannot prompt for", async () => {
  expect(readHostOutcome("codex", "PreToolUse", await gateEmit("block")).effect).toBe("deny");
  const ask = await gateEmit("ask");
  expect(() => readHostOutcome("codex", "PreToolUse", ask)).toThrow(HostOutputError);
});

test.concurrent("exit 2 is a deny carrying stderr on every command host", async () => {
  const blocked = await run(["sh", "-c", "echo 'blocked by hook' >&2; exit 2"]);
  for (const host of ["claude", "codex", "cursor"] as const) {
    expect(readHostOutcome(host, "PreToolUse", blocked)).toEqual({
      effect: "deny",
      reason: "blocked by hook",
    });
  }
});

test.concurrent("claude and codex read the post-tool and legacy block shapes as deny", () => {
  const block = result(JSON.stringify({ decision: "block", reason: "lint failed" }));
  expect(readHostOutcome("claude", "PostToolUse", block)).toEqual({
    effect: "deny",
    reason: "lint failed",
  });
  expect(readHostOutcome("codex", "PreToolUse", block)).toEqual({
    effect: "deny",
    reason: "lint failed",
  });
});

test.concurrent("systemMessage and additionalContext both surface as context", () => {
  const out = result(
    JSON.stringify({
      systemMessage: "note",
      hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: "ctx" },
    }),
  );
  expect(readHostOutcome("claude", "SessionStart", out)).toEqual({
    effect: "allow",
    context: "ctx\n\nnote",
  });
});

test.concurrent("a hookEventName that disagrees with the event is a contract violation", async () => {
  const out = await gateEmit("block");
  expect(() => readHostOutcome("claude", "PostToolUse", out)).toThrow("hookEventName");
});

test.concurrent("non-JSON stdout and a nonzero non-2 exit are contract violations", () => {
  expect(() => readHostOutcome("claude", "PreToolUse", result("not json"))).toThrow(
    HostOutputError,
  );
  expect(() => readHostOutcome("claude", "PreToolUse", result("", 1, "crash"))).toThrow("exited 1");
  expect(() => readHostOutcome("claude", "PreToolUse", result("[1]"))).toThrow(HostOutputError);
});

test.concurrent("cursor: permission objects map to effects and empty stdout is rejected", () => {
  const deny = result(
    JSON.stringify({ permission: "deny", user_message: "u", agent_message: "a" }),
  );
  expect(readHostOutcome("cursor", "beforeShellExecution", deny)).toEqual({
    effect: "deny",
    reason: "a",
  });
  const ask = result(JSON.stringify({ continue: true, permission: "ask", user_message: "u" }));
  expect(readHostOutcome("cursor", "beforeMCPExecution", ask)).toEqual({
    effect: "ask",
    reason: "u",
  });
  const allow = result(JSON.stringify({ permission: "allow" }));
  expect(readHostOutcome("cursor", "preToolUse", allow)).toEqual({ effect: "allow" });
  expect(() => readHostOutcome("cursor", "preToolUse", result(""))).toThrow(HostOutputError);
  expect(() => readHostOutcome("cursor", "preToolUse", result('{"permission":"maybe"}'))).toThrow(
    HostOutputError,
  );
});

test.concurrent("opencode: the evaluated permission effect is the outcome", () => {
  expect(readOpencodeOutcome({ effect: "ask", message: "m" })).toEqual({
    effect: "ask",
    reason: "m",
  });
  expect(readOpencodeOutcome({ effect: "allow" })).toEqual({ effect: "allow" });
  expect(() => readOpencodeOutcome({ effect: "maybe" })).toThrow(HostOutputError);
});
