/**
 * PostToolUse parity harness (#259): one hook call through the bash dispatcher
 * (`post-tools/mod.sh`) and through the committed TypeScript bundle behind its
 * generated launcher, as Claude Code or Codex spawns them. Post-tool modules
 * exist to write state, so each run also reports the project's host state
 * files (gate file, push-review waivers, telemetry) with timestamps normalised.
 * Use `fromSameState` from `./pretool.ts` to run both from one starting point.
 */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { TOOLU_PLUGIN, type PretoolRun } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";
import { run, type RunResult } from "./spawn.ts";

/** The bash command hooks.json ran before #259. */
export const POST_MOD_SH = join(TOOLU_PLUGIN, "hooks/post-tools/mod.sh");

export type PosttoolResult = RunResult & { state: Record<string, string> };

const STAMP = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z/g;

function files(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

/** Every file under the project's `.claude/` and `.codex/`, ISO timestamps as `<T>`. */
export function projectState(sb: Sandbox): Record<string, string> {
  const paths = [".claude", ".codex"].flatMap((dir) => files(join(sb.project, dir)));
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

/** `bash post-tools/mod.sh`, the hooks.json command before #259. */
export function runPostModSh(sb: Sandbox, call: PretoolRun): Promise<PosttoolResult> {
  return withState(sb, run(["bash", POST_MOD_SH], call));
}

/** The hooks.json launcher, which execs the committed `hooks/dist/post-tools.js`. */
export function runPostBundle(sb: Sandbox, call: PretoolRun): Promise<PosttoolResult> {
  const command = launcherCommand({ plugin: "toolu", event: "PostToolUse", entry: "post-tools" });
  return withState(sb, run(["/bin/sh", "-c", command], call));
}
