/**
 * The bash and jq behaviours toolu's lifecycle hooks were written against
 * (#263), kept in one place so the Bun port reproduces them byte for byte.
 */
import { accessSync, constants } from "node:fs";
import { isJsonObject } from "@toolu/core/config";
import { toJqJson } from "@toolu/core/state";

/** `$(…)`: command substitution drops every trailing newline. */
export function stripTrailingNewlines(text: string): string {
  return text.replace(/\n+$/, "");
}

/** `jq -r` of one value: strings raw, anything else as pretty JSON. */
export function jqRaw(value: unknown): string {
  return typeof value === "string" ? value : toJqJson(value, true);
}

/** jq `.key // fallback`: null, false and absent all take the fallback. */
export function jqAlt(value: unknown, fallback: string): string {
  return value === undefined || value === null || value === false
    ? fallback
    : stripTrailingNewlines(jqRaw(value));
}

/** `tr '[:upper:]' '[:lower:]'` under the C locale: ASCII letters only. */
export function asciiLower(text: string): string {
  return text.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

/** `command -v name` over `path`. */
export function onPath(name: string, path: string): boolean {
  return Bun.which(name, { PATH: path }) !== null;
}

/** `[ -x path ]`. */
export function isExecutable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/** `git rev-parse --show-toplevel 2>/dev/null || true` in `cwd`. */
export function gitToplevel(cwd: string, env: Record<string, string | undefined>): string {
  const res = Bun.spawnSync(["git", "rev-parse", "--show-toplevel"], {
    cwd,
    env,
    stdout: "pipe",
    stderr: "ignore",
  });
  return res.exitCode === 0 ? stripTrailingNewlines(res.stdout.toString()) : "";
}

/** Parse hook stdin; `undefined` where jq would fail or see no input. */
export function parseStdin(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

/** `.key` of a parsed JSON document, jq-style: only objects have members. */
export function member(doc: unknown, key: string): unknown {
  return isJsonObject(doc) ? doc[key] : undefined;
}
