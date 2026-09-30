#!/usr/bin/env bun
/**
 * The Claude Code statusline. Reads the statusline JSON on stdin and prints one
 * line with no trailing newline. Settings run the stable path the SessionStart
 * hook publishes, `<config root>/statusline/statusline.sh`, directly:
 *   "statusLine": { "type": "command", "command": "~/.claude/statusline/statusline.sh" }
 * Every field is read defensively: the payload omits effort, used_percentage
 * and more before the first API call, after /compact, or on some models.
 */
import { envValue, type HostEnv } from "@toolu/core/host";
import {
  collectStatus,
  emptyStatus,
  statusHost,
  type ProjectStatus,
} from "./statusline/collect.ts";
import { asObject, readObject } from "./statusline/json.ts";
import { readPayload } from "./statusline/payload.ts";
import { renderLine } from "./statusline/render.ts";

/**
 * The domain of the OAuth email Claude Code stores in `.claude.json` under
 * `CLAUDE_CONFIG_DIR` (never falling back to HOME when that is set, so a
 * custom profile cannot show another account). Only the domain is shown.
 */
function accountDomain(env: HostEnv): string {
  const dir = envValue(env, "CLAUDE_CONFIG_DIR") ?? env["HOME"] ?? "";
  const account = asObject(readObject(`${dir}/.claude.json`)?.["oauthAccount"]);
  const email = account?.["emailAddress"];
  return typeof email === "string" && email.includes("@")
    ? email.slice(email.indexOf("@") + 1)
    : "";
}

function projectStatus(cwd: string, env: HostEnv): ProjectStatus {
  const host = statusHost(env);
  try {
    return collectStatus(cwd, env, host);
  } catch {
    return emptyStatus(host, cwd);
  }
}

const payload = readPayload(await Bun.stdin.text());
process.stdout.write(
  renderLine(payload, projectStatus(payload.cwd, process.env), accountDomain(process.env)),
);
