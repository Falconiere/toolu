/**
 * Tools the acceptance run needs before any host session (#362). A missing or
 * broken tool fails the run up front, named, instead of turning into a skipped
 * or confusing scenario later. Each found tool's version goes into the report.
 */
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";

type ToolReport = { tools: Record<string, string>; missing: string[] };

/** Packing, sandboxes and the ast-grep scenarios need these. */
const REQUIRED = ["git", "npm", "tar", "ast-grep"];
const VERSION_TIMEOUT_MS = 60_000;

async function version(bin: string, env: EnvPatch): Promise<string | null> {
  const res = await run([bin, "--version"], { env, timeoutMs: VERSION_TIMEOUT_MS });
  if (res.exitCode !== 0 || res.timedOut) return null;
  return (res.stdout.trim() || res.stderr.trim()).split("\n")[0] ?? "";
}

async function probeTool(name: string, env: EnvPatch, report: ToolReport): Promise<void> {
  const bin = Bun.which(name, { PATH: env.PATH ?? process.env.PATH ?? "" });
  const found = bin === null ? null : await version(bin, env);
  if (bin === null || found === null) {
    report.missing.push(bin === null ? `${name} (not on PATH)` : `${name} (--version failed)`);
    return;
  }
  report.tools[name] = found;
}

/** Every required tool's version, and a line for each one that is missing or broken. */
export async function preflight(env: EnvPatch = {}): Promise<ToolReport> {
  const report: ToolReport = { tools: {}, missing: [] };
  await Promise.all(REQUIRED.map((name) => probeTool(name, env, report)));
  report.missing.sort();
  return report;
}
