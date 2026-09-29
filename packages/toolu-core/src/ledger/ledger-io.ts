/**
 * Ledger file I/O and location (#256): `pl_read_ledger`, `pl_write_ledger`
 * and `pl_ledger_path`, plus the git questions they ask. Writes produce `jq .`
 * bytes through a `<file>.tmp.<pid>` stage and a rename, exactly like bash.
 * The file keeps the umask mode that a shell redirect gives it.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { gitToplevel, projectStateRoot } from "../host/host-roots.ts";
import { childEnv, type HostEnv, type HostName } from "../host/host-name.ts";
import { branchSlug } from "../state/state-git.ts";
import { toJqJson } from "../state/state-io.ts";
import { parseJson, type Json } from "./ledger-jq.ts";

/** Where a command runs, and what it can see. */
export type LedgerOptions = {
  env?: HostEnv;
  host?: HostName;
  /** The directory bash would have been invoked from. */
  cwd?: string;
  /** One clock for every timestamp and the orphan cutoff. */
  now?: () => Date;
  /** Live tap on each stderr line as it is emitted; the returned text is unchanged. */
  onStderr?: (line: string) => void;
};

export type ReadLedger = { value: Json; text: string };

/**
 * `pl_read_ledger FILE`: the parsed ledger and its text as `$(cat)` captured
 * it. Undefined for an absent or empty file, unparseable JSON, or a document
 * that `jq -e .` rejects (`null`, `false`).
 */
export function readLedger(file: string): ReadLedger | undefined {
  let text: string;
  try {
    if (statSync(file).size === 0) return undefined;
    text = readFileSync(file, "utf8");
  } catch {
    return undefined;
  }
  const value = parseJson(text);
  if (value === undefined || value === null || value === false) return undefined;
  return { value, text: text.replace(/\n+$/, "") };
}

/** `pl_write_ledger FILE JSON`: undefined on success, else the tagged failure line. */
export function writeLedger(file: string, ledger: Json): string | undefined {
  const dir = dirname(file);
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    return `plan-ledger-parse: cannot create ledger dir: ${dir}`;
  }
  const tmp = `${file}.tmp.${process.pid}`;
  // Serialized before the write, so only an I/O failure is reported as a staging failure.
  const body = `${toJqJson(ledger, true)}\n`;
  try {
    writeFileSync(tmp, body);
  } catch {
    rmSync(tmp, { force: true });
    return `plan-ledger-parse: failed to stage ledger to ${tmp}`;
  }
  try {
    renameSync(tmp, file);
  } catch {
    rmSync(tmp, { force: true });
    return `plan-ledger-parse: atomic mv failed for ${file}`;
  }
  return undefined;
}

/** `detect_project_root`: the git toplevel of the invocation directory. */
export function projectRoot(options: LedgerOptions = {}): string | undefined {
  return gitToplevel(options.env ?? process.env, options.cwd);
}

/**
 * `git rev-parse --abbrev-ref HEAD` from the invocation directory, or
 * undefined when it exits non-zero (outside a repo, or an unborn HEAD).
 */
export function headBranch(options: LedgerOptions = {}): string | undefined {
  const res = spawnSync("git", ["rev-parse", "--abbrev-ref", "HEAD"], {
    cwd: options.cwd ?? process.cwd(),
    env: childEnv(options.env ?? process.env),
    encoding: "utf8",
  });
  return res.error === undefined && res.status === 0 ? res.stdout.replace(/\n+$/, "") : undefined;
}

/** `toolu_project_state_root ROOT`: `<root>/<host dir>/tmp`, resolved by the host layer. */
export function stateRootFor(root: string, options: LedgerOptions = {}): string {
  const env = options.env ?? process.env;
  const host = options.host === undefined ? { env } : { env, host: options.host };
  // An explicit root always resolves; the fallback only satisfies the optional return type.
  return projectStateRoot({ ...host, root }) ?? join(root, "tmp");
}

/** `toolu_project_state_dir NAME ROOT`. */
export function stateDirFor(name: string, root: string, options: LedgerOptions = {}): string {
  return join(stateRootFor(root, options), name);
}

/** `pl_ledger_path`: `<root>/<host dir>/tmp/plan-ledger/<branch slug>.json`. */
export function ledgerPath(options: LedgerOptions = {}): string | undefined {
  const root = projectRoot(options);
  if (root === undefined) return undefined;
  const branch = headBranch(options);
  if (branch === undefined) return undefined;
  return join(stateDirFor("plan-ledger", root, options), `${branchSlug(branch)}.json`);
}

/** A command's result: exactly the text bash would print, and its exit code. */
export type CommandResult = { exitCode: 0 | 1 | 2; stdout: string; stderr: string };

/** Collects a command's output; stderr lines are also streamed to `onStderr`. */
export class Output {
  private out = "";
  private readonly err: string[] = [];

  constructor(private readonly onStderr?: (line: string) => void) {}

  stdout(text: string): void {
    this.out += text;
  }

  stderr(line: string): void {
    this.err.push(line);
    this.onStderr?.(line);
  }

  result(exitCode: 0 | 1 | 2): CommandResult {
    return { exitCode, stdout: this.out, stderr: this.err.map((line) => `${line}\n`).join("") };
  }
}
