import { mkdtempSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { bundlePath, resolveEntryCommand } from "../../harness/entry-command.ts";
import type { SuiteOutcome } from "../types.ts";
import { bridgeEnvClaude, repoRoot, tmpBase } from "./helpers.ts";

export type ProtectedDispatchContext = {
  projectRoot: string;
  envPath: string;
  envBefore?: string;
  failPrefix: string;
};

const Fixture = z.object({ toolName: z.string(), sessionId: z.string(), toolCallId: z.string() });
const HookOutput = z.object({
  hookSpecificOutput: z.object({ permissionDecision: z.string() }).optional(),
});

const SELECTOR_KEYS = new Set(["TOOLU_IMPL", "TOOLU_RUST_BIN_DIR"]);

/** Selector controls stay in the harness; the hook sees the environment either implementation would. */
function hookParentEnv(): Record<string, string | undefined> {
  return Object.fromEntries(Object.entries(process.env).filter(([key]) => !SELECTOR_KEYS.has(key)));
}

async function executeNative(
  argv: string[],
  root: string,
  projectRoot: string,
  payload: Record<string, unknown>,
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const pluginRoot = join(root, "plugins", "toolu");
  const home = mkdtempSync(join(tmpBase(), "toolu-conformance-claude-home-"));
  const proc = Bun.spawn(argv, {
    cwd: projectRoot,
    env: {
      ...hookParentEnv(),
      ...bridgeEnvClaude(root),
      HOME: home,
      TOOLU_CONFIG_DIR: home,
      CLAUDE_PROJECT_DIR: projectRoot,
      CLAUDE_PLUGIN_ROOT: pluginRoot,
    },
    stdin: new Blob([JSON.stringify(payload)]),
    stdout: "pipe",
    stderr: "pipe",
  });
  const timer = setTimeout(() => proc.kill(), 15_000);
  try {
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ]);
    return { stdout, stderr, exitCode };
  } finally {
    clearTimeout(timer);
  }
}

type HookCommand = { argv: string[]; failPrefix: string };

/** The pre-tools command `TOOLU_IMPL` selects; a Rust run names itself in every failure. */
function hookCommand(root: string, failPrefix: string): HookCommand | SuiteOutcome {
  try {
    const command = resolveEntryCommand({
      plugin: "toolu",
      entry: "pre-tools",
      bundle: bundlePath(join(root, "plugins", "toolu"), "pre-tools"),
    });
    const prefix =
      command.implementation === "rust" ? `${failPrefix} [rust:toolu/pre-tools]` : failPrefix;
    return { argv: command.argv, failPrefix: prefix };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { status: "fail", message: `${failPrefix}: setup failed: ${message}` };
  }
}

/** A protected .env edit must be denied or asked, and a deny must leave the bytes alone. */
function judgeDecision(
  stdout: string,
  failPrefix: string,
  ctx: ProtectedDispatchContext,
): SuiteOutcome {
  let decision: string | undefined;
  try {
    const loaded: unknown = JSON.parse(stdout);
    decision = HookOutput.parse(loaded).hookSpecificOutput?.permissionDecision;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: "fail",
      message: `${failPrefix}: native dispatch output invalid: ${message}`,
    };
  }
  if (decision !== "deny" && decision !== "ask") {
    return {
      status: "fail",
      message: `${failPrefix}: expected deny or ask for protected .env edit, got ${decision ?? "allow"}`,
    };
  }
  if (
    decision === "deny" &&
    ctx.envBefore !== undefined &&
    readFileSync(ctx.envPath, "utf8") !== ctx.envBefore
  ) {
    return { status: "fail", message: `${failPrefix}: deny path mutated protected .env bytes` };
  }
  return { status: "pass" };
}

/** Claude's committed native PreToolUse bundle on the protected-file fixture. */
export async function runProtectedEditDispatch(
  ctx: ProtectedDispatchContext,
): Promise<SuiteOutcome> {
  const root = repoRoot();
  const fixturePath = join(root, "fixtures/portable-core/protected-files-pre.json");
  let fixtureRaw: z.infer<typeof Fixture>;
  try {
    const loaded: unknown = JSON.parse(readFileSync(fixturePath, "utf8"));
    fixtureRaw = Fixture.parse(loaded);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { status: "fail", message: `${ctx.failPrefix}: fixture read failed: ${message}` };
  }
  const payload = {
    tool_name: fixtureRaw.toolName,
    tool_input: { file_path: ctx.envPath },
    session_id: fixtureRaw.sessionId,
    tool_use_id: fixtureRaw.toolCallId,
    cwd: ctx.projectRoot,
  };
  const command = hookCommand(root, ctx.failPrefix);
  if ("status" in command) return command;
  const { stdout, stderr, exitCode } = await executeNative(
    command.argv,
    root,
    ctx.projectRoot,
    payload,
  );
  if (exitCode !== 0) {
    return {
      status: "fail",
      message: `${command.failPrefix}: native dispatch exited ${exitCode}: ${stderr}`,
    };
  }
  return judgeDecision(stdout, command.failPrefix, ctx);
}
