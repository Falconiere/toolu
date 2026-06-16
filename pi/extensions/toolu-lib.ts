import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { ExtensionContext, ToolCallEvent, ToolResultEvent } from "@earendil-works/pi-coding-agent";
import type { HookOutput } from "./toolu.js";

const gateStatusKey = "toolu-gate";

/** Resolve the pi coding-agent config dir (`$PI_CODING_AGENT_DIR` or `~/.pi/agent`). */
export function agentDir(): string {
  return process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent");
}

/** Build the child-process env toolu hooks run under, scoping config to the agent dir. */
export function baseEnv(cwd: string): NodeJS.ProcessEnv {
  const dir = agentDir();
  mkdirSync(dir, { recursive: true });
  return {
    ...process.env,
    TOOLU_CONFIG_DIR: dir,
    CLAUDE_CONFIG_DIR: dir,
    PI_CODING_AGENT_DIR: dir,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".pi",
    TOOLU_RUNTIME: "pi",
    PWD: cwd,
  };
}

/** Resolve the git toplevel for `cwd`, falling back to `cwd` when it is not a repo. */
export function projectRoot(cwd: string): string {
  try {
    return (
      execFileSync("git", ["-C", cwd, "rev-parse", "--show-toplevel"], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      }).trim() || cwd
    );
  } catch (error) {
    process.stderr.write(
      `toolu: projectRoot falling back to cwd, not a git repo (${error instanceof Error ? error.message : String(error)})\n`,
    );
    return cwd;
  }
}

/** Path to the project's quality-gate status file under `.claude/tmp`. */
export function gateFile(cwd: string): string {
  return join(projectRoot(cwd), ".claude", "tmp", "quality-gate-status.json");
}

/** Map a pi tool name to its Claude Code equivalent, or undefined when unmapped. */
export function mappedToolName(toolName: string): string | undefined {
  switch (toolName) {
    case "bash":
      return "Bash";
    case "read":
      return "Read";
    case "edit":
      return "Edit";
    case "write":
      return "Write";
    case "grep":
      return "Grep";
    case "find":
    case "ls":
      return "Glob";
    default:
      return undefined;
  }
}

/** Normalize a pi tool event's input into the tool_input shape toolu hooks expect. */
export function toolInputForEvent(
  event: ToolCallEvent | ToolResultEvent,
): Record<string, unknown> {
  switch (event.toolName) {
    case "bash":
      return {
        command: event.input.command,
        timeout: event.input.timeout,
      };
    case "read":
      return {
        path: event.input.path,
        file_path: event.input.path,
        offset: event.input.offset,
        limit: event.input.limit,
      };
    case "edit":
      return {
        path: event.input.path,
        file_path: event.input.path,
        target_file: event.input.path,
        edits: event.input.edits,
      };
    case "write":
      return {
        path: event.input.path,
        file_path: event.input.path,
        target_file: event.input.path,
      };
    case "grep":
      return { ...event.input };
    case "find":
    case "ls":
      return { ...event.input };
    default:
      return { ...event.input };
  }
}

/** Serialize a tool_call event into the JSON payload for the pre-tool hook. */
export function toolCallPayload(event: ToolCallEvent): string {
  return JSON.stringify({
    tool_name: mappedToolName(event.toolName),
    tool_input: toolInputForEvent(event),
  });
}

/** Extract a bash command's exit code from a tool_result event, or undefined if absent. */
export function parseBashExitCode(event: ToolResultEvent): number | undefined {
  if (event.toolName !== "bash") return undefined;
  if (!event.isError) return 0;
  const text = (event.content || [])
    .filter((part): part is { type: "text"; text: string } => part.type === "text")
    .map((part) => part.text)
    .join("\n");
  const match = text.match(/Command exited with code (\d+)/);
  if (match) return Number(match[1]);
  return undefined;
}

/** Serialize a tool_result event into the JSON payload for the post-tool hook. */
export function toolResultPayload(event: ToolResultEvent): string {
  const exitCode = parseBashExitCode(event);
  return JSON.stringify({
    tool_name: mappedToolName(event.toolName),
    tool_input: toolInputForEvent(event),
    tool_response: exitCode === undefined ? undefined : { metadata: { exit_code: exitCode } },
    tool_output: exitCode === undefined ? undefined : { exitCode },
  });
}

/** Run a toolu hook script as a child process, returning its exit code, stdout, and stderr. */
export async function runHook(
  script: string,
  cwd: string,
  input: string,
  signal?: AbortSignal,
): Promise<{ code: number; stdout: string; stderr: string }> {
  return new Promise((resolve, _reject) => {
    const child = spawn("bash", [script], {
      cwd,
      env: baseEnv(cwd),
      stdio: ["pipe", "pipe", "pipe"],
      signal,
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (chunk) => {
      stdout += String(chunk);
    });
    child.stderr.on("data", (chunk) => {
      stderr += String(chunk);
    });

    const handleError = (error: Error) => {
      stderr += error.message;
    };

    child.on("error", handleError);
    child.on("close", (code) => {
      resolve({ code: code ?? 1, stdout: stdout.trim(), stderr: stderr.trim() });
    });

    child.stdin.end(input);
  });
}

function isHookOutput(value: unknown): value is HookOutput {
  return typeof value === "object" && value !== null;
}

function isGateStatus(value: unknown): value is { status?: string; reason?: string } {
  return typeof value === "object" && value !== null;
}

/** Parse a hook's stdout into a HookOutput, or undefined when it is empty or not JSON. */
export function parseHookOutput(stdout: string): HookOutput | undefined {
  if (!stdout) return undefined;
  let parsed: unknown;
  try {
    parsed = JSON.parse(stdout);
  } catch (error) {
    process.stderr.write(
      `toolu: ignoring non-JSON hook output (${error instanceof Error ? error.message : String(error)})\n`,
    );
    return undefined;
  }
  return isHookOutput(parsed) ? parsed : undefined;
}

/** Collapse a HookOutput's system message and additional context into display text. */
export function hookText(output: HookOutput | undefined): string | undefined {
  if (!output) return undefined;
  const parts = [output.systemMessage, output.hookSpecificOutput?.additionalContext].filter(
    (value): value is string => Boolean(value),
  );
  if (parts.length === 0) return undefined;
  return parts.join("\n\n");
}

/** Update the UI status line to reflect the project's current quality-gate state. */
export function refreshGateStatus(ctx: ExtensionContext) {
  if (!ctx.hasUI) return;
  const theme = ctx.ui.theme;
  const file = gateFile(ctx.cwd);
  if (!existsSync(file)) {
    ctx.ui.setStatus(gateStatusKey, theme.fg("dim", "gate: clear"));
    return;
  }

  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    process.stderr.write(
      `toolu: gate status unreadable (${error instanceof Error ? error.message : String(error)})\n`,
    );
    ctx.ui.setStatus(gateStatusKey, theme.fg("warning", "gate: unreadable"));
    return;
  }
  if (isGateStatus(raw) && raw.status === "failing") {
    const reason = raw.reason ? ` — ${raw.reason}` : "";
    ctx.ui.setStatus(gateStatusKey, theme.fg("warning", `gate: failing${reason}`));
    return;
  }

  ctx.ui.setStatus(gateStatusKey, theme.fg("success", "gate: clear"));
}

/** Run each installed plugin's register.sh to sync its hook modules into the pi runtime registry. */
export async function runRegistrySync(cwd: string, scripts: string[]) {
  const env = baseEnv(cwd);
  mkdirSync(join(agentDir(), "toolu", "pre-tools.d"), { recursive: true });
  mkdirSync(join(agentDir(), "toolu", "post-tools.d"), { recursive: true });

  for (const script of scripts) {
    if (!existsSync(script)) continue;
    await new Promise<void>((resolve) => {
      const child = spawn("bash", [script], {
        cwd,
        env,
        stdio: ["pipe", "ignore", "ignore"],
      });
      child.on("close", () => resolve());
      child.on("error", () => resolve());
      child.stdin.end("{}");
    });
  }
}
