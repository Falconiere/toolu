/**
 * What byte-savings records for one tool call, measured exactly as the bash
 * module's jq pipeline did: the kind of tool, the bytes its response put into
 * context, and for a single-file Read the full size of that file.
 */
import { statSync } from "node:fs";
import { basename, resolve } from "node:path";
import { analyzeShell } from "@toolu/core/shell";
import { jqOr, jqRaw, jqToString } from "./jq-text.ts";

export type SavingsKind = "read" | "grep" | "glob" | "ast-grep";

const TOOL_KINDS: Readonly<Record<string, SavingsKind>> = {
  Read: "read",
  Grep: "grep",
  Glob: "glob",
};

const AST_GREP_BINARIES = new Set(["ast-grep", "sg"]);

/** True when some command the shell would run is `ast-grep` or `sg`, wrappers and `bash -c` included. */
function runsAstGrep(command: string): boolean {
  return analyzeShell(command).commands.some((cmd) => {
    const program = cmd.argv?.[0];
    return typeof program === "string" && AST_GREP_BINARIES.has(basename(program));
  });
}

export function savingsKind(toolName: string, command: unknown): SavingsKind | undefined {
  const kind = TOOL_KINDS[toolName];
  if (kind !== undefined) return kind;
  if (toolName !== "Bash" && toolName !== "Shell") return undefined;
  return runsAstGrep(jqRaw(jqOr(command, ""))) ? "ast-grep" : undefined;
}

/** `.tool_response | if string then . elif object then (.content? // .stdout? // .output? // tostring) else tostring end`. */
function responseValue(response: unknown): unknown {
  if (typeof response === "string") return response;
  if (typeof response !== "object" || response === null || Array.isArray(response)) {
    return jqToString(response ?? null);
  }
  const field = (key: string): unknown => Reflect.get(response, key);
  return jqOr(field("content"), jqOr(field("stdout"), jqOr(field("output"), jqToString(response))));
}

/** UTF-8 byte length of the response text, trailing newlines excluded; undefined when there is no text. */
export function returnedBytes(response: unknown): number | undefined {
  const text = jqRaw(responseValue(response));
  return text === "" ? undefined : Buffer.byteLength(text, "utf8");
}

/** Size of the regular file a Read named, relative paths against `cwd`; 0 otherwise. */
export function readFullBytes(filePath: unknown, cwd: string): number {
  const path = jqRaw(jqOr(filePath, ""));
  if (path === "") return 0;
  try {
    const stat = statSync(resolve(cwd, path));
    return stat.isFile() ? stat.size : 0;
  } catch {
    return 0;
  }
}

/** `.session_id // "unknown"`, keeping only letters, digits and dashes. */
export function ledgerSessionId(sessionId: unknown): string {
  const sid = jqRaw(jqOr(sessionId, "unknown")).replace(/[^A-Za-z0-9-]/gu, "");
  return sid === "" ? "unknown" : sid;
}
