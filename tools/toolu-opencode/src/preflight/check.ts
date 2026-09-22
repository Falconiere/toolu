/** Prerequisite probe for OpenCode bootstrap (#211). */
import { statSync } from "node:fs";
import { join } from "node:path";

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

const ALLOWED_BINARY = /^[A-Za-z0-9._+-]+$/;

/** PATH lookup without shell — binary must be a simple tool name. */
function commandPresent(binary: string, env: Record<string, string>): boolean {
  if (!ALLOWED_BINARY.test(binary)) {
    return false;
  }
  const pathEnv = env.PATH ?? process.env.PATH ?? "";
  for (const dir of pathEnv.split(":")) {
    if (dir.length === 0) {
      continue;
    }
    const candidate = join(dir, binary);
    try {
      if (statSync(candidate).isFile()) {
        return true;
      }
    } catch {
      continue;
    }
  }
  return false;
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

export function runPreflight(options?: { env?: Record<string, string> }): PreflightReport {
  const env = { ...stringEnv(process.env), ...options?.env };
  const tools: PreflightTool[] = ["bash", "jq", "git", "bun", "opencode"];
  const entries: PreflightEntry[] = tools.map((tool) => ({
    tool,
    present: commandPresent(tool, env),
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
