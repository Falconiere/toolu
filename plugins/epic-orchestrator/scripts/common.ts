/** Shared helpers for epic-orchestrator CLIs (Bun + stdlib only). */

import { mkdir, rename, writeFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const PLUGIN_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const REF_DIR = join(PLUGIN_ROOT, "skills", "epic-orchestrator", "references");
export const SCRIPTS_DIR = join(PLUGIN_ROOT, "scripts");

export const EPICS_HOME = process.env.EPIC_STATE_HOME ?? join(homedir(), ".claude", "epics");

const REF_URL = /github\.com\/([^/\s]+)\/([^/\s]+)\/(?:issues|pull)\/(\d+)/;
const REF_SHORT = /^([\w.-]+)\/([\w.-]+)#(\d+)$/;

export class CommandError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CommandError";
  }
}

export async function run(
  cmd: string[],
  opts: { cwd?: string; check?: boolean } = {},
): Promise<string> {
  const check = opts.check ?? true;
  const spawnOpts: { cwd?: string; stdout: "pipe"; stderr: "pipe" } = {
    stdout: "pipe",
    stderr: "pipe",
  };
  if (opts.cwd !== undefined) spawnOpts.cwd = opts.cwd;
  const proc = Bun.spawn(cmd, spawnOpts);
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (check && code !== 0) {
    throw new CommandError(`${cmd.join(" ")} failed (${code}): ${(stderr || stdout).trim()}`);
  }
  return stdout;
}

export async function ghJson(args: string[], opts: { cwd?: string } = {}): Promise<unknown> {
  const out = await run(["gh", ...args], opts);
  return out.trim() ? JSON.parse(out) : null;
}

export async function herdr(args: string[]): Promise<Record<string, unknown>> {
  const proc = Bun.spawn(["herdr", ...args], { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  await proc.exited;
  const text = stdout.trim() || stderr.trim();
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new CommandError(`herdr ${args.join(" ")}: non-JSON output: ${text.slice(0, 300)}`);
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new CommandError(`herdr ${args.join(" ")}: expected object JSON`);
  }
  const data: Record<string, unknown> = { ...parsed };
  if ("error" in data && data.error && typeof data.error === "object" && data.error !== null) {
    const err = data.error as { code?: unknown; message?: unknown };
    throw new CommandError(`herdr ${args.join(" ")}: ${String(err.code)}: ${String(err.message)}`);
  }
  if (
    data.result &&
    typeof data.result === "object" &&
    data.result !== null &&
    !Array.isArray(data.result)
  ) {
    return { ...data.result };
  }
  return data;
}

export function parseRef(ref: string, defaultRepo?: string | null): [string, string, number] {
  const trimmed = ref.trim();
  const url = REF_URL.exec(trimmed);
  if (url && url[1] && url[2] && url[3]) {
    return [url[1], url[2], Number(url[3])];
  }
  const short = REF_SHORT.exec(trimmed);
  if (short && short[1] && short[2] && short[3]) {
    return [short[1], short[2], Number(short[3])];
  }
  const bare = trimmed.replace(/^#/, "");
  if (/^\d+$/.test(bare)) {
    const repo = defaultRepo ?? currentRepoSync();
    const slash = repo.indexOf("/");
    if (slash < 1) throw new Error(`bad default_repo: ${repo}`);
    return [repo.slice(0, slash), repo.slice(slash + 1), Number(bare)];
  }
  throw new Error(`unrecognized issue reference: ${JSON.stringify(ref)}`);
}

function currentRepoSync(): string {
  const proc = Bun.spawnSync([
    "gh",
    "repo",
    "view",
    "--json",
    "nameWithOwner",
    "-q",
    ".nameWithOwner",
  ]);
  if (proc.exitCode !== 0) {
    const errBuf = proc.stderr;
    const errText = typeof errBuf === "undefined" ? "" : Buffer.from(errBuf).toString();
    throw new CommandError(`gh repo view failed: ${errText}`);
  }
  const outBuf = proc.stdout;
  return (typeof outBuf === "undefined" ? "" : Buffer.from(outBuf).toString()).trim();
}

export async function currentRepo(): Promise<string> {
  return (
    await run(["gh", "repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"])
  ).trim();
}

/** Stable id that is also a valid herdr agent name: [a-z][a-z0-9_-]{0,31}.
 * Pass `owner/repo` (preferred) or a bare repo name. Owner is folded into the
 * slug so `orgA/foo#1` and `orgB/foo#1` do not collide. */
export function issueKey(repo: string, number: number): string {
  let base =
    repo
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "") || "repo";
  if (!/^[a-z]/.test(base)) base = `r${base}`;
  const suffix = `-${number}`;
  return `${base.slice(0, 32 - suffix.length).replace(/-+$/, "")}${suffix}`;
}

export function slugify(title: string, words = 5): string {
  const cleaned = title.replace(/^\s*\[[^\]]*\]\s*/, "");
  const parts = cleaned.toLowerCase().match(/[a-z0-9]+/g) ?? [];
  return parts.slice(0, words).join("-") || "work";
}

export function stateDirFor(owner: string, repo: string, number: number): string {
  return join(EPICS_HOME, `${owner}-${repo}-${number}`.toLowerCase());
}

export function readJson<T>(path: string, fallback: T): T {
  try {
    return JSON.parse(readFileSync(path, "utf8")) as T;
  } catch {
    return fallback;
  }
}

export async function writeJson(path: string, data: unknown): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const tmp = `${path}.tmp`;
  await writeFile(tmp, `${JSON.stringify(data, null, 2)}\n`);
  await rename(tmp, path);
}

export function die(msg: string, code = 1): never {
  process.stderr.write(`${JSON.stringify({ error: msg })}\n`);
  process.exit(code);
}
