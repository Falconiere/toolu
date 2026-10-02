/**
 * Prompt and compaction stdout (#341). The real toolu bundles produce the
 * JSON; parseHookContext accepts that event, and parseStartupOutput still
 * rejects anything that is not SessionStart.
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { parseHookContext } from "../hook-context.ts";
import { parseStartupOutput } from "../output.ts";
import { PLUGINS_ROOT, tempRoot } from "./fixtures.ts";

const PROMPT_BUNDLE = join(PLUGINS_ROOT, "toolu", "hooks", "dist", "user-prompt-submit.js");
const START_BUNDLE = join(PLUGINS_ROOT, "toolu", "hooks", "dist", "session-start.js");

const RENAME = "Rename: find all refs (ast-grep + Grep on configs) before rewriting.";
const VAGUE = "Prompt too vague - specify what file/feature/error needs attention";

async function bundleStdout(bundle: string, stdin: string, cwd: string): Promise<string> {
  const home = join(cwd, "home");
  mkdirSync(home, { recursive: true });
  const proc = Bun.spawn([process.execPath, bundle], {
    cwd,
    env: {
      PATH: process.env.PATH ?? "",
      HOME: home,
      XDG_CONFIG_HOME: join(cwd, "xdg"),
      TOOLU_CONFIG_DIR: join(cwd, "config"),
      TOOLU_HOST_OVERRIDE: "opencode",
      TOOLU_BUN: process.execPath,
    },
    stdin: new Blob([stdin]),
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (code !== 0) throw new Error(`exit ${String(code)}: ${stderr}`);
  return stdout;
}

test("a real rename prompt yields the rename reminder", async () => {
  using root = tempRoot("toolu-hook-ctx-");
  const stdout = await bundleStdout(
    PROMPT_BUNDLE,
    JSON.stringify({ prompt: "rename the parser in src/parser.ts" }),
    root.path,
  );
  const parsed = parseHookContext(stdout, "UserPromptSubmit");
  expect(parsed.ok).toBe(true);
  if (!parsed.ok || parsed.context.kind !== "context") throw new Error("expected context");
  expect(parsed.context.additionalContext).toContain(RENAME);
  expect(parseStartupOutput(stdout).ok).toBe(false);
  expect(parseHookContext(stdout, "SessionStart").ok).toBe(false);
});

test("a trivial reply and a vague verb stay distinct", async () => {
  using root = tempRoot("toolu-hook-ctx-");
  const ok = parseHookContext(
    await bundleStdout(PROMPT_BUNDLE, JSON.stringify({ prompt: "ok" }), root.path),
    "UserPromptSubmit",
  );
  expect(ok).toEqual({ ok: true, context: { kind: "context" } });
  const fix = parseHookContext(
    await bundleStdout(PROMPT_BUNDLE, JSON.stringify({ prompt: "fix" }), root.path),
    "UserPromptSubmit",
  );
  expect(fix.ok).toBe(true);
  if (!fix.ok || fix.context.kind !== "block") throw new Error("expected block");
  expect(fix.context.reason).toBe(VAGUE);
});

test("compact SessionStart keeps the post-compaction instructions", async () => {
  using root = tempRoot("toolu-hook-ctx-");
  const stdout = await bundleStdout(
    START_BUNDLE,
    JSON.stringify({ hook_event_name: "SessionStart", source: "compact", cwd: root.path }),
    root.path,
  );
  const parsed = parseHookContext(stdout, "SessionStart");
  expect(parsed.ok).toBe(true);
  if (!parsed.ok || parsed.context.kind !== "context") throw new Error("expected context");
  expect(parsed.context.additionalContext).toContain("Recover memories before continuing");
  expect(parsed.context.systemMessage ?? "").toContain("Context compacted");
  expect(parsed.context.additionalContext ?? "").not.toBe(parsed.context.systemMessage);
  expect(parseStartupOutput(stdout).ok).toBe(true);
});

test("empty, non-JSON, and a combined payload follow the contract", () => {
  expect(parseHookContext("", "UserPromptSubmit")).toEqual({
    ok: true,
    context: { kind: "context" },
  });
  expect(parseHookContext("not json", "PreCompact").ok).toBe(false);
  expect(parseHookContext(JSON.stringify({ decision: "block" }), "UserPromptSubmit").ok).toBe(
    false,
  );
  const both = parseHookContext(
    JSON.stringify({
      hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: "keep" },
      decision: "block",
      reason: "nope",
    }),
    "UserPromptSubmit",
  );
  expect(both).toEqual({
    ok: true,
    context: { kind: "context", additionalContext: "keep" },
  });
});
