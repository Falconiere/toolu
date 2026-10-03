/**
 * Shared by the core (#358) and delivery (#355) workflow suites: a git project
 * with a local bare remote, the before hook's refusal of a bash command, and a
 * bash call run with toolu's `shell.env` and shown to the after hook.
 */
import { join } from "node:path";
import type { Hooks } from "@opencode-ai/plugin";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import type { HostBinding } from "../context.ts";
import { createTooluHooks } from "../hooks.ts";
import { bashEnv, hook, inShell } from "./jev-fixtures.ts";

const CALL = { sessionID: "ses_workflow", callID: "call_workflow" };

export type Project = {
  branch: string;
  selection: object;
  gates: object;
  /** Files committed on `branch`, one commit ahead of `main`. */
  files: Record<string, string>;
};

/**
 * `sb` (created with `git: true`) with `main` pushed to a bare remote, `branch`
 * one commit ahead, and the project's toolu selection and gates. Returns the
 * remote's path.
 */
export function gitProject(sb: Sandbox, project: Project): string {
  const remote = join(sb.root, "remote.git");
  sb.write(".gitignore", ".opencode/\nnode_modules/\n");
  sb.write("package.json", {
    name: "workflow-project",
    private: true,
    scripts: { test: "bun test" },
  });
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "chore: project");
  sb.git("init", "-q", "--bare", remote);
  sb.git("remote", "add", "origin", remote);
  sb.git("push", "-q", "origin", "main");
  sb.git("checkout", "-q", "-b", project.branch);
  for (const [path, text] of Object.entries(project.files)) sb.write(path, text);
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "feat: change");
  sb.write(".opencode/toolu/plugins.json", project.selection);
  sb.write(".opencode/toolu.config.json", { version: 1, gates: project.gates });
  return remote;
}

/** The before hook's refusal for a bash `command`, or "allowed". */
export async function refusal(hooks: Hooks, command: string): Promise<string> {
  try {
    await hook(hooks, "tool.execute.before")({ tool: "bash", ...CALL }, { args: { command } });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return "allowed";
}

/** `command` run in the agent's bash, with its result shown to the after hook as OpenCode would. */
export async function bash(hooks: Hooks, sb: Sandbox, command: string) {
  const res = await inShell(sb, command, await bashEnv(hooks, sb));
  const input = { tool: "bash", ...CALL, args: { command } };
  const output = {
    title: "bash",
    output: res.stdout + res.stderr,
    metadata: { exit: res.exitCode },
  };
  await hook(hooks, "tool.execute.after")(input, output);
  return res;
}

/** The remote's `branch` head, or "" before the first push. */
export function remoteHead(sb: Sandbox, remote: string, branch: string): string {
  try {
    return sb.git("--git-dir", remote, "rev-parse", `refs/heads/${branch}`).trim();
  } catch {
    return "";
  }
}

export async function withHooks(
  b: HostBinding,
  body: (hooks: Hooks) => Promise<void>,
): Promise<void> {
  const hooks = await createTooluHooks(b);
  try {
    await body(hooks);
  } finally {
    await hooks.dispose?.();
  }
}
