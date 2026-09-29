/**
 * Stable-path publishing (#269). A host exports `CLAUDE_PLUGIN_ROOT` to hook
 * subprocesses only, never to the agent's shell, so a skill cannot name a file
 * inside the plugin. A SessionStart hook symlinks it to
 * `<config root>/<dir>/<name>` instead, which the agent's shell can expand.
 * Port of the symlink block every `session-start.sh` repeated.
 */
import { randomUUID } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  readlinkSync,
  renameSync,
  rmSync,
  statSync,
  symlinkSync,
} from "node:fs";
import { join } from "node:path";
import { envValue, type HostEnv } from "../host/host-name.ts";
import { configRoot } from "../host/host-roots.ts";

export type PublishOptions = {
  /** Plugin name, the prefix of every stderr line. */
  plugin: string;
  /** Absolute path of the file to publish. */
  source: string;
  /** Directory under the config root, e.g. `context7`. */
  dir: string;
  /** Published file name, e.g. `search.sh`. */
  name: string;
  /** Word used in the cannot-create line: `wrapper` (default) or `helper`. */
  what?: string;
  env?: HostEnv;
  /** Receives the cannot-create line; default writes it to stderr. */
  warn?: (line: string) => void;
};

export type PublishResult =
  | { status: "published" | "kept-user-file" | "link-failed"; path: string }
  | { status: "unwritable"; path: string }
  | { status: "source-missing" };

function stderrLine(line: string): void {
  process.stderr.write(`${line}\n`);
}

/** `[ -f path ]`: a regular file, following symlinks. */
function isFile(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isFile() ?? false;
}

/** A fresh symlink beside `dst`, renamed over it: readers never see the path missing. */
function relink(source: string, dst: string): boolean {
  const tmp = `${dst}.${randomUUID()}.tmp`;
  try {
    symlinkSync(source, tmp);
    renameSync(tmp, dst);
    return true;
  } catch {
    rmSync(tmp, { force: true });
    return false;
  }
}

/**
 * Symlink `source` at `<config root>/<dir>/<name>`. The path is owned only
 * when it is absent or already a symlink (stale or broken ones are replaced);
 * a regular file or directory there is the user's and is never touched.
 * Never throws: a missing source is a corrupted install and publishes
 * nothing, silently.
 */
export function publishWrapper(options: PublishOptions): PublishResult {
  if (!isFile(options.source)) {
    return { status: "source-missing" };
  }
  const env = options.env ?? process.env;
  const dir = join(configRoot({ env }), options.dir);
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    (options.warn ?? stderrLine)(
      `${options.plugin}: cannot create ${dir} — ${options.what ?? "wrapper"} not published`,
    );
    return { status: "unwritable", path: dir };
  }
  const path = join(dir, options.name);
  const existing = lstatSync(path, { throwIfNoEntry: false });
  if (existing !== undefined && !existing.isSymbolicLink()) {
    return { status: "kept-user-file", path };
  }
  if (existing !== undefined && readlinkSync(path) === options.source) {
    return { status: "published", path };
  }
  return { status: relink(options.source, path) ? "published" : "link-failed", path };
}

/** `command -v bun` over `env.PATH`. */
export function bunOnPath(env: HostEnv = process.env): boolean {
  return Bun.which("bun", { PATH: envValue(env, "PATH") ?? "" }) !== null;
}

/** The one-line advisory a published Bun CLI prints at SessionStart when `bun` is not on PATH. */
export function bunAdvisory(plugin: string, tool: string): string {
  return `${plugin}: bun not found on PATH — the ${tool} needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)`;
}

/**
 * Publish a Bun CLI bundle run through its `#!/usr/bin/env bun` shebang. Once
 * the publish step ran (a source to link and a directory to link in), warn
 * when `bun` is off PATH: every call through the published path would fail
 * with a bare "env: bun: No such file". Advisory; the session continues.
 */
export function publishBunCli(options: PublishOptions & { tool: string }): PublishResult {
  const result = publishWrapper(options);
  const ran = result.status !== "source-missing" && result.status !== "unwritable";
  if (ran && !bunOnPath(options.env ?? process.env)) {
    (options.warn ?? stderrLine)(bunAdvisory(options.plugin, options.tool));
  }
  return result;
}
