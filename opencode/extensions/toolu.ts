import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const extensionDir = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(extensionDir, "../..");

const preToolsScript = join(packageRoot, "plugins/toolu/hooks/pre-tools/mod.sh");
const postToolsScript = join(packageRoot, "plugins/toolu/hooks/post-tools/mod.sh");
const preCompactScript = join(packageRoot, "plugins/toolu/hooks/pre-compact.sh");

const registerScripts = [
  join(packageRoot, "plugins/ast-grep/hooks/register.sh"),
  join(packageRoot, "plugins/comemory/hooks/register.sh"),
  join(packageRoot, "plugins/ts-quality/hooks/register.sh"),
  join(packageRoot, "plugins/rust-quality/hooks/register.sh"),
  join(packageRoot, "plugins/git-better/hooks/register.sh"),
];

// opencode plugin API surface. Defined locally so typecheck works without
// installing @opencode-ai/plugin. The published type package mirrors this
// shape verbatim per the opencode plugin docs.
type PluginInput = {
  project: { worktree?: string };
  client: unknown;
  $: { (strings: TemplateStringsArray, ...values: unknown[]): Promise<{ stdout: string; stderr: string; exitCode: number }> };
  directory: string;
  worktree: string;
};

type ToolExecuteInput = { tool: string; callID?: string };
type ToolExecuteBeforeOutput = { args: Record<string, unknown>; metadata?: Record<string, unknown> };
type ToolExecuteAfterOutput = { title: string; output: string; metadata: Record<string, unknown> };
type CompactingOutput = { context: string[]; prompt: string };

type Hooks = {
  "session.created"?: (input: { sessionID?: string }) => Promise<void>;
  "tool.execute.before"?: (
    input: ToolExecuteInput,
    output: ToolExecuteBeforeOutput,
  ) => Promise<void>;
  "tool.execute.after"?: (
    input: ToolExecuteInput,
    output: ToolExecuteAfterOutput,
  ) => Promise<void>;
  "experimental.session.compacting"?: (
    input: { sessionID?: string },
    output: CompactingOutput,
  ) => Promise<void>;
};

type Plugin = (input: PluginInput) => Promise<Hooks>;

type HookOutput = {
  systemMessage?: string;
  hookSpecificOutput?: {
    additionalContext?: string;
    permissionDecision?: string;
    permissionDecisionReason?: string;
  };
  decision?: string;
  reason?: string;
};

/** Resolve the opencode config dir (`$OPENCODE_CONFIG_DIR` or `~/.config/opencode`). */
export function agentDir(): string {
  return process.env.OPENCODE_CONFIG_DIR || join(homedir(), ".config", "opencode");
}

/** Build the child-process env toolu hooks run under, scoping config to the opencode agent dir. */
export function baseEnv(cwd: string): NodeJS.ProcessEnv {
  const dir = agentDir();
  mkdirSync(dir, { recursive: true });
  return {
    ...process.env,
    TOOLU_CONFIG_DIR: dir,
    OPENCODE_CONFIG_DIR: dir,
    TOOLU_RUNTIME: "opencode",
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
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

/** Path to the project's quality-gate status file under `.opencode/tmp`. */
export function gateFile(cwd: string): string {
  return join(projectRoot(cwd), ".opencode", "tmp", "quality-gate-status.json");
}

/** Map an opencode tool name to its Claude Code equivalent, or undefined when unmapped. */
export function mappedToolName(toolName: string): string | undefined {
  switch (toolName) {
    case "bash":
      return "Bash";
    case "read":
      return "Read";
    case "edit":
    case "patch":
      return "Edit";
    case "write":
      return "Write";
    case "grep":
      return "Grep";
    case "glob":
    case "list":
      return "Glob";
    case "webfetch":
      return "WebFetch";
    case "websearch":
      return "WebSearch";
    default:
      return undefined;
  }
}

/** Tools whose result should be inspected by the post-tools gate. */
export function isPostToolTarget(toolName: string): boolean {
  return toolName === "bash" || toolName === "edit" || toolName === "write" || toolName === "patch";
}

/** Normalize an opencode tool event's args into the tool_input shape toolu hooks expect. */
export function toolInputForEvent(input: ToolExecuteInput, args: Record<string, unknown>): Record<string, unknown> {
  const toolName = input.tool;
  switch (toolName) {
    case "bash": {
      const out: Record<string, unknown> = {};
      if (typeof args.command === "string") out.command = args.command;
      if (typeof args.description === "string") out.description = args.description;
      if (typeof args.timeout === "number") out.timeout = args.timeout;
      return out;
    }
    case "read": {
      const out: Record<string, unknown> = {};
      const filePath = (args.filePath ?? args.path) as string | undefined;
      if (filePath) {
        out.path = filePath;
        out.file_path = filePath;
      }
      if (typeof args.offset === "number") out.offset = args.offset;
      if (typeof args.limit === "number") out.limit = args.limit;
      return out;
    }
    case "edit":
    case "patch": {
      const out: Record<string, unknown> = {};
      const filePath = (args.filePath ?? args.path) as string | undefined;
      if (filePath) {
        out.path = filePath;
        out.file_path = filePath;
        out.target_file = filePath;
      }
      if (Array.isArray(args.edits)) out.edits = args.edits;
      if (typeof args.oldString === "string") out.old_string = args.oldString;
      if (typeof args.newString === "string") out.new_string = args.newString;
      return out;
    }
    case "write": {
      const out: Record<string, unknown> = {};
      const filePath = (args.filePath ?? args.path) as string | undefined;
      if (filePath) {
        out.path = filePath;
        out.file_path = filePath;
        out.target_file = filePath;
      }
      if (typeof args.content === "string") out.content = args.content;
      return out;
    }
    case "grep":
      return { ...args };
    case "glob":
    case "list":
      return { ...args };
    default:
      return { ...args };
  }
}

/** Serialize a tool.execute.before event into the JSON payload for the pre-tool hook. */
export function toolCallPayload(input: ToolExecuteInput, args: Record<string, unknown>): string {
  return JSON.stringify({
    tool_name: mappedToolName(input.tool),
    tool_input: toolInputForEvent(input, args),
  });
}

/** Serialize a tool.execute.after event into the JSON payload for the post-tool hook. */
export function toolResultPayload(input: ToolExecuteInput, args: Record<string, unknown>, result: string, isError: boolean): string {
  // toolu modules inspect tool_input.command + tool_input.file_path only; the
  // result metadata is exposed via tool_response / tool_output for modules that
  // want to key on exit code. We do not have an exit code from opencode for
  // non-bash tools; emit isError as a boolean in tool_output.
  return JSON.stringify({
    tool_name: mappedToolName(input.tool),
    tool_input: toolInputForEvent(input, args),
    tool_response: isError ? { is_error: true } : { is_error: false },
    tool_output: isError ? { isError: true } : { isError: false },
  });
}

/** Run a toolu hook script as a child process, returning its exit code, stdout, and stderr. */
export async function runHook(
  script: string,
  cwd: string,
  input: string,
  signal?: AbortSignal,
): Promise<{ code: number; stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
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

/** Read the current gate status from the project gate file. */
export function readGateStatus(cwd: string): { status?: string; reason?: string } | undefined {
  const file = gateFile(cwd);
  if (!existsSync(file)) return undefined;
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    process.stderr.write(
      `toolu: gate status unreadable (${error instanceof Error ? error.message : String(error)})\n`,
    );
    return { status: "unreadable" };
  }
  return isGateStatus(raw) ? raw : undefined;
}

/** Format the gate status as a single-line summary suitable for the /toolu-status command. */
export function gateStatusText(cwd: string): string {
  const status = readGateStatus(cwd);
  if (!status) return "gate: clear";
  if (status.status === "failing") {
    const reason = status.reason ? ` — ${status.reason}` : "";
    return `gate: failing${reason}`;
  }
  if (status.status === "unreadable") return "gate: unreadable";
  return "gate: clear";
}

/** Run each installed plugin's register.sh to sync its hook modules into the opencode runtime registry. */
export async function runRegistrySync(cwd: string): Promise<void> {
  const env = baseEnv(cwd);
  mkdirSync(join(agentDir(), "toolu", "pre-tools.d"), { recursive: true });
  mkdirSync(join(agentDir(), "toolu", "post-tools.d"), { recursive: true });

  for (const script of registerScripts) {
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

/** Compose a gate-status block for the /toolu-status command output. */
function gateStatusBlock(cwd: string): string {
  const status = readGateStatus(cwd);
  if (!status) return "[toolu-gate] gate: clear";
  if (status.status === "failing") {
    return `[toolu-gate] gate: failing — ${status.reason ?? "unknown"}`;
  }
  if (status.status === "unreadable") {
    return "[toolu-gate] gate: status file unreadable";
  }
  return "[toolu-gate] gate: clear";
}

/** opencode plugin entry point: wires toolu pre/post-tool hooks and gate status into the agent. */
const tooluOpencodePlugin: Plugin = async ({ directory, worktree }) => {
  const cwd = worktree || directory;

  return {
    "session.created": async () => {
      await runRegistrySync(cwd);
    },

    "tool.execute.before": async (input, output) => {
      const name = mappedToolName(input.tool);
      if (!name || !existsSync(preToolsScript)) return;

      // Thread the original args through metadata so tool.execute.after can
      // rebuild the tool_input payload. opencode's after-event does not
      // surface the original args, only the result.
      const args = output.args || {};
      if (!output.metadata) output.metadata = {};
      (output.metadata as Record<string, unknown>).tooluArgs = args;

      const result = await runHook(preToolsScript, cwd, toolCallPayload(input, args));

      if (result.code === 2) {
        throw new Error(result.stderr || result.stdout || "Blocked by toolu pre-tool hook");
      }

      const parsed = parseHookOutput(result.stdout);
      if (parsed?.hookSpecificOutput?.permissionDecision === "deny") {
        throw new Error(
          parsed.hookSpecificOutput.permissionDecisionReason || "Blocked by toolu pre-tool hook",
        );
      }

      if (parsed?.decision === "block") {
        throw new Error(parsed.reason || "Blocked by toolu pre-tool hook");
      }

      const note = hookText(parsed);
      if (note) {
        (output.metadata as Record<string, unknown>).tooluNote = note;
      }
    },

    "tool.execute.after": async (input, output) => {
      if (!isPostToolTarget(input.tool)) return;
      if (!existsSync(postToolsScript)) return;

      const args =
        (output.metadata as { tooluArgs?: Record<string, unknown> } | undefined)?.tooluArgs || {};
      const isError = Boolean((output.metadata as { isError?: boolean } | undefined)?.isError);

      const result = await runHook(
        postToolsScript,
        cwd,
        toolResultPayload(input, args, output.output || "", isError),
      );

      const parsed = parseHookOutput(result.stdout);

      if (result.code !== 0 && result.stderr) {
        output.output = `${output.output || ""}\n[toolu]\n${result.stderr}`.trim();
      }

      const text = hookText(parsed);
      if (text) {
        output.output = `${output.output || ""}\n[toolu]\n${text}`.trim();
      }

      // Surface the current gate status after every gated tool run. opencode
      // has no statusline bar; this is the closest inline equivalent.
      const status = readGateStatus(cwd);
      if (status?.status === "failing") {
        const reason = status.reason ? ` — ${status.reason}` : "";
        output.output = `${output.output}\n[toolu-gate] gate: failing${reason}`.trim();
      } else if (status?.status === "unreadable") {
        output.output = `${output.output}\n[toolu-gate] gate: status file unreadable`.trim();
      }
    },

    "experimental.session.compacting": async (_input, output) => {
      if (!existsSync(preCompactScript)) return;
      const result = await runHook(preCompactScript, cwd, "{}");
      if (result.stdout) {
        output.context.push(`[toolu pre-compact]\n${result.stdout}`);
      }
    },
  };
};

export default tooluOpencodePlugin;

export const __testing = {
  gateStatusText,
  readGateStatus,
  isPostToolTarget,
  toolResultPayload,
  gateStatusBlock,
};
