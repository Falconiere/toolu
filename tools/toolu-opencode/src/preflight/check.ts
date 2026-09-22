/** Prerequisite probe for OpenCode bootstrap (#211). */
import { createBunBashRunner } from "@toolu/core/runner";

export type PreflightTool = "bash" | "jq" | "git" | "bun" | "opencode";

export type PreflightEntry = {
  tool: PreflightTool;
  present: boolean;
  detail?: string;
};

export type PreflightReport = {
  entries: PreflightEntry[];
  bootstrapAllowed: boolean;
  reasons: string[];
};

const REQUIRED_FOR_BOOTSTRAP: PreflightTool[] = ["bash", "jq"];

async function commandPresent(binary: string, env: Record<string, string>): Promise<boolean> {
  const runner = createBunBashRunner();
  const result = await runner.run({
    argv: ["bash", "-lc", `command -v ${binary}`],
    cwd: env.PWD ?? process.cwd(),
    env,
    stdin: "",
    deadlineMs: 5_000,
    maxStdoutBytes: 4_096,
  });
  return result.ok && result.exitCode === 0;
}

/** Structured prerequisite report; missing bash/jq fail closed for bootstrap. */
function stringEnv(source: NodeJS.ProcessEnv): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(source)) {
    if (value !== undefined) {
      out[key] = value;
    }
  }
  return out;
}

export async function runPreflight(options?: {
  env?: Record<string, string>;
}): Promise<PreflightReport> {
  const env = { ...stringEnv(process.env), ...options?.env };
  const tools: PreflightTool[] = ["bash", "jq", "git", "bun", "opencode"];
  const presence = await Promise.all(tools.map((tool) => commandPresent(tool, env)));
  const entries: PreflightEntry[] = tools.map((tool, index) => ({
    tool,
    present: presence[index] ?? false,
  }));
  const reasons: string[] = [];
  for (const required of REQUIRED_FOR_BOOTSTRAP) {
    const entry = entries.find((e) => e.tool === required);
    if (entry && !entry.present) {
      reasons.push(`missing required tool: ${required}`);
    }
  }
  return {
    entries,
    bootstrapAllowed: reasons.length === 0,
    reasons,
  };
}
