/**
 * Tools the acceptance run needs before any host session (#362). A missing or
 * broken tool fails the run up front, named, instead of turning into a skipped
 * or confusing scenario later. Each found tool's version goes into the report.
 */
import { existsSync } from "node:fs";
import { z } from "zod";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";

type ToolReport = { tools: Record<string, string>; missing: string[] };

/** Packing, sandboxes, the ast-grep scenarios and the browser workflow need these. */
const REQUIRED = ["git", "npm", "tar", "ast-grep", "agent-browser"];
const VERSION_TIMEOUT_MS = 60_000;

const Doctor = z.object({
  checks: z.array(z.looseObject({ id: z.string(), status: z.string(), message: z.string() })),
});

async function version(bin: string, env: EnvPatch): Promise<string | null> {
  const res = await run([bin, "--version"], { env, timeoutMs: VERSION_TIMEOUT_MS });
  if (res.exitCode !== 0 || res.timedOut) return null;
  return (res.stdout.trim() || res.stderr.trim()).split("\n")[0] ?? "";
}

/** Malformed doctor output reads like no report at all, so the caller reports Chromium missing. */
function jsonOrNull(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** The Chromium agent-browser drives, from its own doctor report; null when it is not installed. */
async function chromium(bin: string, env: EnvPatch): Promise<string | null> {
  const res = await run([bin, "doctor", "--json"], { env, timeoutMs: VERSION_TIMEOUT_MS });
  if (res.exitCode !== 0 || res.timedOut) return null;
  const parsed = Doctor.safeParse(jsonOrNull(res.stdout));
  const installed = parsed.success
    ? parsed.data.checks.find((check) => check.id === "chrome.installed")
    : undefined;
  const path = installed?.message.split(" at ").at(-1);
  if (installed?.status !== "pass" || path === undefined || !existsSync(path)) return null;
  return installed.message;
}

async function probeTool(name: string, env: EnvPatch, report: ToolReport): Promise<void> {
  const bin = Bun.which(name, { PATH: env.PATH ?? process.env.PATH ?? "" });
  const found = bin === null ? null : await version(bin, env);
  if (bin === null || found === null) {
    report.missing.push(bin === null ? `${name} (not on PATH)` : `${name} (--version failed)`);
    return;
  }
  report.tools[name] = found;
  if (name !== "agent-browser") return;
  const browser = await chromium(bin, env);
  if (browser === null) report.missing.push("Chromium (run agent-browser install)");
  else report.tools.chromium = browser;
}

/** Every required tool's version, and a line for each one that is missing or broken. */
export async function preflight(env: EnvPatch = {}): Promise<ToolReport> {
  const report: ToolReport = { tools: {}, missing: [] };
  await Promise.all(REQUIRED.map((name) => probeTool(name, env, report)));
  report.missing.sort();
  return report;
}
