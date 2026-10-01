/** Run the committed PostToolUse bundle and capture its project state. */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import type { PretoolRun } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";
import { run, type RunResult } from "./spawn.ts";

export type PosttoolResult = RunResult & { state: Record<string, string> };

const STAMP = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z/g;

function files(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

/**
 * Every file under the project's `.claude/` and `.codex/` (and those of each
 * directory in `also`, e.g. a second worktree), keyed relative to the project,
 * ISO timestamps as `<T>`.
 */
export function projectState(sb: Sandbox, also: readonly string[] = []): Record<string, string> {
  const roots = [sb.project, ...also];
  const paths = roots.flatMap((root) =>
    [".claude", ".codex"].flatMap((dir) => files(join(root, dir))),
  );
  return Object.fromEntries(
    paths
      .toSorted()
      .map((path) => [
        relative(sb.project, path),
        readFileSync(path, "utf8").replace(STAMP, "<T>"),
      ]),
  );
}

async function withState(sb: Sandbox, result: Promise<RunResult>): Promise<PosttoolResult> {
  return { ...(await result), state: projectState(sb) };
}

/** The hooks.json launcher, which execs the committed `hooks/dist/post-tools.js`. */
export function runPostBundle(sb: Sandbox, call: PretoolRun): Promise<PosttoolResult> {
  const command = launcherCommand({ plugin: "toolu", event: "PostToolUse", entry: "post-tools" });
  return withState(sb, run(["/bin/sh", "-c", command], call));
}
