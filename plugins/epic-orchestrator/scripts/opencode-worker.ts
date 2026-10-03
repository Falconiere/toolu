/** What an OpenCode worker launch needs beyond herdr: a 1.x `opencode` on PATH
 * and toolu's runtime state kept out of the worktree's git status. */

import { existsSync } from "node:fs";
import { appendFile, mkdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { CommandError, run } from "./common.ts";
import { opencodeModelArgs, opencodeVersionProblem } from "./hosts.ts";

/** toolu's OpenCode runtime state inside a project: the data root and the
 * gate, ledger and telemetry tmp tree. User-owned `.opencode` files stay visible. */
export const OPENCODE_EXCLUDE = ["/.opencode/toolu/state/", "/.opencode/tmp/"];
const EXCLUDE_MARKER = "# toolu epic-orchestrator: OpenCode worker runtime state";

/** Append OPENCODE_EXCLUDE to the checkout's shared `info/exclude`, which every
 * linked worktree reads, so snapshots, leftovers and `git add -A` skip it.
 * Idempotent; returns the exclude file's path. */
export async function excludeOpencodeState(checkout: string): Promise<string> {
  const common = await run([
    "git",
    "-C",
    checkout,
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ]);
  const path = join(common.trim(), "info", "exclude");
  const current = existsSync(path) ? await readFile(path, "utf8") : "";
  const present = new Set(current.split("\n"));
  const missing = [EXCLUDE_MARKER, ...OPENCODE_EXCLUDE].filter((line) => !present.has(line));
  if (missing.length === 0) return path;
  await mkdir(dirname(path), { recursive: true });
  const lead = current === "" || current.endsWith("\n") ? "" : "\n";
  await appendFile(path, `${lead}${missing.join("\n")}\n`);
  return path;
}

/** Exclude the worker's state (dry-run writes nothing); returns the launch log line. */
export async function excludeForWorker(checkout: string, dry: boolean): Promise<string> {
  if (!dry) await excludeOpencodeState(checkout);
  return `# opencode: exclude ${OPENCODE_EXCLUDE.join(" ")} in ${checkout}'s info/exclude`;
}

/** Refuse a model the TUI cannot take, then (live runs only) an `opencode`
 * release toolu's plugin does not target: herdr types `opencode` into the pane. */
export async function checkOpencode(model: string | undefined, dry: boolean): Promise<void> {
  if (model !== undefined) opencodeModelArgs(model);
  if (dry) return;
  let output: string;
  try {
    output = await run(["opencode", "--version"]);
  } catch (err) {
    const reason = (err instanceof Error ? err.message : String(err)).slice(0, 200);
    throw new CommandError(
      `opencode is not runnable here (${reason}); toolu's OpenCode plugin targets opencode-ai 1.x. ` +
        "Put a 1.x opencode first on PATH, or route this issue to another host.",
    );
  }
  const problem = opencodeVersionProblem(output);
  if (problem !== null) throw new CommandError(problem);
}
