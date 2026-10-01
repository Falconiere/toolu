/** Prerequisite probe for OpenCode bootstrap (#211). */
import { accessSync, constants, statSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";

export type PreflightTool = "git" | "bun" | "opencode";

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

const REQUIRED_FOR_BOOTSTRAP: PreflightTool[] = ["git", "bun"];

const ALLOWED_BINARY = /^[A-Za-z0-9._+-]+$/;

function executableFile(path: string): boolean {
  try {
    if (!statSync(path).isFile()) return false;
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/** PATH lookup without shell — binary must be a simple tool name. */
function commandPath(binary: string, env: Record<string, string>): string | null {
  if (!ALLOWED_BINARY.test(binary)) return null;
  for (const dir of (env.PATH ?? "").split(delimiter)) {
    if (dir.length === 0) {
      continue;
    }
    const candidate = resolve(dir, binary);
    if (executableFile(candidate)) return candidate;
  }
  return null;
}

/** Match the hook launcher's TOOLU_BUN, PATH, then ~/.bun/bin/bun order. */
export function resolveBunExecutable(env: Record<string, string>): string | null {
  if (env.TOOLU_BUN && executableFile(env.TOOLU_BUN)) return resolve(env.TOOLU_BUN);
  const fromPath = commandPath("bun", env);
  if (fromPath) return fromPath;
  const fromHome = env.HOME ? join(env.HOME, ".bun", "bin", "bun") : null;
  return fromHome && executableFile(fromHome) ? resolve(fromHome) : null;
}

/** Structured prerequisite report; missing git or Bun fails closed. */
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
  const tools: PreflightTool[] = ["git", "bun", "opencode"];
  const entries: PreflightEntry[] = tools.map((tool) => ({
    tool,
    present: tool === "bun" ? resolveBunExecutable(env) !== null : commandPath(tool, env) !== null,
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
