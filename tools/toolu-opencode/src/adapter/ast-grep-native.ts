/** Run ast-grep's compiled rules for OpenCode's in-process dispatch path. */
import { existsSync } from "node:fs";
import { join } from "node:path";
import type { ModuleResult } from "@toolu/core/dispatch";

const TIMEOUT_MS = 30_000;
const QUIET: ModuleResult = { exitCode: 0, stdout: "", stderr: "" };

export type NativeRuleOptions = {
  configRoot: string;
  cwd: string;
  env: Record<string, string>;
  selectedPluginSpecs?: ReadonlySet<string> | undefined;
};

/** A missing manifest means SessionStart has not enabled this rule. */
export async function astGrepRule(
  phase: "pre-tools" | "post-tools",
  payload: Record<string, unknown>,
  options: NativeRuleOptions,
): Promise<ModuleResult> {
  if (
    options.selectedPluginSpecs !== undefined &&
    !options.selectedPluginSpecs.has("ast-grep@toolu")
  )
    return QUIET;
  const entry = phase === "pre-tools" ? "search-nudge" : "byte-savings";
  const folder = phase === "pre-tools" ? "pre-tools.d" : "post-tools.d";
  if (!existsSync(join(options.configRoot, "toolu", folder, `ast-grep@toolu__${entry}.json`)))
    return QUIET;
  const tools =
    phase === "pre-tools" ? ["Grep", "Bash", "Shell"] : ["Read", "Grep", "Glob", "Bash", "Shell"];
  if (typeof payload.tool_name !== "string" || !tools.includes(payload.tool_name)) return QUIET;
  const event = phase === "pre-tools" ? "PreToolUse" : "PostToolUse";
  let proc: Bun.Subprocess<Blob, "pipe", "pipe">;
  try {
    proc = Bun.spawn(
      [options.env.TOOLU_BIN ?? "toolu", "ast-grep", "hook", phase, "--event", event],
      {
        cwd: options.cwd,
        env: options.env,
        stdin: new Blob([JSON.stringify(payload)]),
        stdout: "pipe",
        stderr: "pipe",
        timeout: TIMEOUT_MS,
        killSignal: "SIGKILL",
      },
    );
  } catch (error) {
    return {
      exitCode: 1,
      stdout: "",
      stderr: `ast-grep native rule could not start: ${String(error)}`,
    };
  }
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  return proc.killed
    ? { exitCode: 1, stdout: "", stderr: `ast-grep native rule timed out after ${TIMEOUT_MS} ms` }
    : { exitCode, stdout, stderr };
}
