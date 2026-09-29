/**
 * The file a post-edit quality module checks (#265), a port of the preamble the
 * ts/python/rust-quality bash modules share: `FILE_PATH` from
 * `CLAUDE_FILE_PATHS` or the edit tool's input, the delete/move fields the
 * dispatcher adds when it splits a patch, and the linked-worktree skip.
 */
import { spawnSync } from "node:child_process";
import { statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { isJsonObject } from "../config/config-load.ts";
import { childEnv, envValue } from "../host/host-name.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { toJqJson } from "../state/state-io.ts";

/** Tools whose input names the edited file; any other tool relies on `CLAUDE_FILE_PATHS`. */
const EDIT_TOOLS: ReadonlySet<string> = new Set(["Write", "Edit", "MultiEdit"]);

export type EditedFile = {
  /** `FILE_PATH` exactly as the bash module saw it: the gate key and the path in messages. */
  readonly path: string;
  /** `path` resolved against the hook's working directory, for file and git calls. */
  readonly absolute: string;
  /** A deletion or the source side of a move: nothing to lint, only a gate entry to clear. */
  readonly removed: boolean;
};

/** `$(jq -r '.tool_input.<a> // .tool_input.<b> // … // empty')`: first member not null or false. */
function inputField(ctx: RegistryContext, keys: readonly string[]): string {
  const input = ctx.raw.tool_input;
  if (!isJsonObject(input)) return "";
  for (const key of keys) {
    const value = input[key];
    if (value === undefined || value === null || value === false) continue;
    const text = typeof value === "string" ? value : toJqJson(value, true);
    return text.replace(/\n+$/, "");
  }
  return "";
}

/**
 * `${TOOLU_EDIT_<NAME>:-$(jq … .tool_input.toolu_edit_<name> // "")}`. For one
 * path of a split patch the dispatcher exports `ctx.edit` to bash modules as
 * that variable; otherwise it is whatever the hook process inherited.
 */
function editField(
  ctx: RegistryContext,
  split: string | undefined,
  name: "OPERATION" | "MOVED_TO",
): string {
  const exported = ctx.edit === undefined ? envValue(ctx.env, `TOOLU_EDIT_${name}`) : split;
  if (exported !== undefined && exported !== "") return exported;
  return inputField(ctx, [`toolu_edit_${name.toLowerCase()}`]);
}

/** The edited file, or undefined when the payload names none. */
export function editedFile(event: RegistryHookEvent, ctx: RegistryContext): EditedFile | undefined {
  const fromInput = EDIT_TOOLS.has(event.toolName)
    ? inputField(ctx, ["path", "file_path", "target_file"])
    : "";
  const path = envValue(ctx.env, "CLAUDE_FILE_PATHS") ?? fromInput;
  if (path === "") return undefined;
  const operation = editField(ctx, ctx.edit?.operation, "OPERATION");
  const movedTo = editField(ctx, ctx.edit?.movedTo, "MOVED_TO");
  return {
    path,
    absolute: resolve(ctx.cwd ?? process.cwd(), path),
    removed: operation === "delete" || movedTo !== "",
  };
}

/** bash `[ -f "$FILE_PATH" ]`: a regular file, symlinks followed. */
export function isRegularFile(file: EditedFile): boolean {
  try {
    return statSync(file.absolute).isFile();
  } catch {
    return false;
  }
}

/** bash `${dir%/}`. */
function withoutSlash(dir: string): string {
  return dir.replace(/\/$/, "");
}

/**
 * Whether the file sits in a git linked worktree, where quality state is not
 * tracked: `git rev-parse --git-dir --git-common-dir` from its directory
 * disagree. Outside a repository, or when git fails, it is not.
 */
export function inLinkedWorktree(file: EditedFile, ctx: RegistryContext): boolean {
  const res = spawnSync(
    "git",
    [
      "-C",
      dirname(file.path),
      "rev-parse",
      "--path-format=absolute",
      "--git-dir",
      "--git-common-dir",
    ],
    { cwd: ctx.cwd, env: childEnv(ctx.env), encoding: "utf8" },
  );
  const [gitDir = "", commonDir = ""] = res.stdout.split("\n");
  return gitDir !== "" && commonDir !== "" && withoutSlash(gitDir) !== withoutSlash(commonDir);
}
