/**
 * `analyzeShell` (#284): parse a Bash/Shell command once with unbash and walk
 * it into flat records. It never throws. unbash returns a partial AST plus
 * errors for bad input, so the commands it could read are still reported (a
 * `git push` followed by an unterminated quote is still a push). An input over
 * the size cap is not parsed and is reported as unknown.
 */
import { parse } from "unbash";
import type { ShellAnalysis, ShellError } from "./shell-types.ts";
import { walkScript, type WalkContext, type WalkSink } from "./shell-walk.ts";

/** Longest command analyzed, in UTF-16 code units (1 MiB): 1 MiB of dense script parses in ~15 ms. */
export const MAX_SHELL_INPUT = 1024 * 1024;

function unknownAnalysis(source: string, error: ShellError): ShellAnalysis {
  return { source, commands: [], compoundRedirects: [], errors: [error], unknown: true };
}

export function analyzeShell(source: string): ShellAnalysis {
  if (source.length > MAX_SHELL_INPUT) {
    return unknownAnalysis(source, {
      message: `oversize: ${source.length} characters exceeds the ${MAX_SHELL_INPUT} cap`,
      pos: MAX_SHELL_INPUT,
      origin: "line",
    });
  }
  const sink: WalkSink = { commands: [], compoundRedirects: [], errors: [] };
  const root: WalkContext = {
    source,
    origin: "line",
    depth: 0,
    proves: true,
    pipeline: { index: 0, size: 1 },
  };
  try {
    walkScript(parse(source), root, sink);
  } catch (error) {
    // unbash recovers from malformed input by design; a throw is a parser defect.
    // Report it as unknown, which guardrails treat as "ask", never as "nothing runs".
    const message = error instanceof Error ? error.message : String(error);
    return unknownAnalysis(source, { message: `parser: ${message}`, pos: 0, origin: "line" });
  }
  if (sink.errors.length === 0) return { source, ...sink, unknown: false };
  // A line bash cannot parse exits non-zero, so a zero status proves nothing about it.
  const commands = sink.commands.map((command) => ({ ...command, exitProves: false }));
  return { ...sink, source, commands, unknown: commands.length === 0 };
}
