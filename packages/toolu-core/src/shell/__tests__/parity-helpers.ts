/**
 * Shared by the bash-parity suites (#284): run the unmodified shipped bash libs
 * over fixture commands, and compose the same answers from `@toolu/core/shell`.
 * The bash side only reads `plugins/toolu/hooks/lib/detect.sh` and
 * `pre-tools/modules/bash-commands.sh`; nothing under `plugins/` is written.
 */
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { z } from "zod";
import { projectRoot } from "../../host/host-roots.ts";
import { pushTargets, runsGitSubcommand } from "../shell-git.ts";
import { analyzeShell } from "../shell-parse.ts";
import { matchesRule } from "../shell-rules.ts";
import { writeTargets } from "../shell-writes.ts";

export const REPO = resolve(import.meta.dir, "../../../../..");
export const FIXTURES = join(REPO, "tooling/fixtures/shell");
const DETECT_SH = join(REPO, "plugins/toolu/hooks/lib/detect.sh");
const BASH_COMMANDS_SH = join(REPO, "plugins/toolu/hooks/pre-tools/modules/bash-commands.sh");

export type Env = Record<string, string>;

/** A clean environment: no host-session variable leaks into the bash side. */
export function cleanEnv(home: string, extra: Env = {}): Env {
  return { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: home, LC_ALL: "C", ...extra };
}

/** A PATH holding every tool the libs need except python3 (#283 item 4). */
export function pathWithoutPython(dir: string): string {
  const bin = join(dir, "bin-no-python");
  mkdirSync(bin, { recursive: true });
  for (const tool of [
    "bash",
    "jq",
    "awk",
    "grep",
    "sed",
    "tr",
    "cat",
    "head",
    "tail",
    "dirname",
    "basename",
    "mktemp",
    "rm",
    "mkdir",
    "env",
    "uname",
    "cut",
    "sort",
    "wc",
    "git",
  ]) {
    const found = Bun.which(tool);
    if (found !== null) symlinkSync(found, join(bin, tool));
  }
  return bin;
}

/**
 * Source `lib`, then run `body` once per NUL-separated record on stdin. Each
 * record's fields are read into `$f1`, `$f2`, …; each result ends with a NUL.
 */
export function bashBatch(
  lib: "detect" | "bash-commands",
  body: string,
  records: readonly (readonly string[])[],
  env: Env,
  cwd = REPO,
): string[] {
  const fields = records[0]?.length ?? 1;
  const reads = Array.from(
    { length: fields },
    (_, i) => `IFS= read -r -d '' f${i + 1} || break`,
  ).join("; ");
  const source = lib === "detect" ? `. "${DETECT_SH}"` : `tool_name=Bash; . "${BASH_COMMANDS_SH}"`;
  const script = `${source}\nwhile :; do ${reads}; ${body}; printf '\\0'; done`;
  const stdin = records.map((record) => record.map((field) => `${field}\0`).join("")).join("");
  const res = spawnSync("bash", ["-c", script], {
    cwd,
    input: stdin,
    env,
    encoding: "utf8",
    maxBuffer: 64 << 20,
  });
  if (res.status !== 0) throw new Error(`bash exited ${String(res.status)}: ${res.stderr}`);
  return res.stdout.split("\0").slice(0, records.length);
}

export const DecideCase = z.object({
  command: z.string(),
  allow: z.array(z.string()),
  deny: z.array(z.string()),
});
export type DecideCase = z.infer<typeof DecideCase>;

/** `bash_commands_decide` for each case, with its own allow/deny lists on disk. */
export function bashDecide(cases: readonly DecideCase[], dir: string, env: Env): string[] {
  const records = cases.map((c, i) => {
    const settings = join(dir, `settings-${i}`);
    mkdirSync(settings, { recursive: true });
    writeFileSync(join(settings, "bash-allowlist.txt"), `${c.allow.join("\n")}\n`);
    writeFileSync(join(settings, "bash-denylist.txt"), `${c.deny.join("\n")}\n`);
    return [settings, c.command];
  });
  return bashBatch(
    "bash-commands",
    'TOOLU_SETTINGS_DIR="$f1"; bash_commands_decide "$f2"',
    records,
    env,
  ).map((out) => out.trim());
}

export function tsIsGit(command: string, sub: "push" | "commit"): boolean {
  return runsGitSubcommand(analyzeShell(command), sub) === "yes";
}

/** Write targets as `bash_write_targets` prints them: static paths, else the text as written. */
export function tsWriteTargets(command: string): string[] {
  return writeTargets(analyzeShell(command)).map((t) => t.path ?? t.text);
}

/** Deny first, then an allow match overrides, as `bash_commands_decide` composes them. */
export function tsDecide(c: DecideCase): string {
  const { commands } = analyzeShell(c.command);
  const hits = (rule: string) => commands.some((command) => matchesRule(command, rule));
  const denied = c.deny.find(hits);
  if (denied === undefined) return "allow";
  return c.allow.some(hits) ? "allow" : `deny:${denied}`;
}

function gitOut(cwd: string, args: readonly string[], env: Env): string | undefined {
  const res = spawnSync("git", [...args], { cwd, env, encoding: "utf8" });
  const out = res.stdout.trim();
  return res.status === 0 && out !== "" ? out : undefined;
}

/** `push_target_root`'s contract composed from `pushTargets`: the -C chain, then cwd, then the project root. */
export function tsPushRoot(command: string, cwd: string, env: Env): string {
  const chain = pushTargets(analyzeShell(command))[0]?.cChain ?? [];
  const static_ = chain.filter((dir) => dir !== null);
  const viaChain =
    chain.length > 0 && static_.length === chain.length
      ? gitOut(cwd, [...static_.flatMap((dir) => ["-C", dir]), "rev-parse", "--show-toplevel"], env)
      : undefined;
  return (
    viaChain ??
    gitOut(cwd, ["rev-parse", "--show-toplevel"], env) ??
    projectRoot({ env, cwd }) ??
    cwd
  );
}

/** `push_target_branch`: the attached branch, else the refspec destination on a detached HEAD. */
export function tsPushBranch(command: string, root: string, env: Env): string {
  const branch = gitOut(root, ["rev-parse", "--abbrev-ref", "HEAD"], env);
  if (branch !== undefined && branch !== "HEAD") return branch;
  return pushTargets(analyzeShell(command))[0]?.destination ?? "";
}

/** A disposable directory tree, removed by `using`. */
export function scratch(prefix: string): { dir: string; [Symbol.dispose](): void } {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), prefix)));
  return { dir, [Symbol.dispose]: () => rmSync(dir, { recursive: true, force: true }) };
}

export function initRepo(dir: string, branch = "main"): void {
  mkdirSync(dir, { recursive: true });
  const git = (...args: string[]) => spawnSync("git", ["-C", dir, ...args], { encoding: "utf8" });
  git("init", "-q", "-b", branch);
  git("-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init");
}

export function relativeTo(base: string, path: string): string {
  return relative(base, realpathSync(path)) || ".";
}

export function readFixture(name: string): unknown {
  return JSON.parse(readFileSync(join(FIXTURES, name), "utf8"));
}
