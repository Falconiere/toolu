/**
 * What the post-tool gates read from the host's payload (#259), exactly as
 * `gate-status.sh` and `push-waiver.sh` read it with `jq -r`: the command, the
 * reported exit status and the interrupt flag. Each value is the text bash
 * would hold in its variable; the native payload tests pin these cases.
 */
import { JqError, alt, get, parseJson, raw, type Json } from "../ledger/ledger-jq.ts";

/** The payload as jq sees it: whatever `JSON.parse` accepted is JSON. */
function asJson(payload: Readonly<Record<string, unknown>>): Json {
  return parseJson(JSON.stringify(payload)) ?? null;
}

/** `jq -r '<paths joined by //> // <fallback>'`: undefined where jq raises. */
function firstOf(
  doc: Json,
  paths: readonly (readonly string[])[],
  fallback: Json,
): Json | undefined {
  try {
    for (const path of paths) {
      const value = path.reduce<Json>((at, key) => get(at, key), doc);
      if (alt(value, null) !== null) return value;
    }
    return fallback;
  } catch (error) {
    if (error instanceof JqError) return undefined;
    throw error;
  }
}

/** `$(jq -r '<expr>' ... || echo "<onError>")`: `// empty` prints nothing, `$(…)` strips newlines. */
function printed(value: Json | undefined, onError: string): string {
  if (value === undefined) return onError;
  if (value === null) return "";
  return raw(value).replace(/\n+$/, "");
}

/** `.tool_input.command // ""`. */
export function toolCommand(payload: Readonly<Record<string, unknown>>): string {
  return printed(firstOf(asJson(payload), [["tool_input", "command"]], ""), "");
}

const EXIT_PATHS = [
  ["tool_response", "metadata", "exit_code"],
  ["tool_response", "exit_code"],
  ["tool_output", "exit_code"],
] as const;

/**
 * The reported exit status: `tool_response.metadata.exit_code`, then
 * `tool_response.exit_code`, then `tool_output.exit_code`, and when that is
 * empty or the text `null`, `exitCode`/`exit_code` inside `tool_output` (a
 * JSON string or object). "" when the host reported none.
 */
export function toolExitStatus(payload: Readonly<Record<string, unknown>>): string {
  const doc = asJson(payload);
  const status = printed(firstOf(doc, EXIT_PATHS, null), "");
  if (status !== "" && status !== "null") return status;
  const output = printed(firstOf(doc, [["tool_output"]], null), "");
  if (output === "") return status;
  const inner = parseJson(output);
  if (inner === undefined) return "";
  return printed(firstOf(inner, [["exitCode"], ["exit_code"]], null), "");
}

/** `.tool_response.interrupted // false` printed as `true`. */
export function toolInterrupted(payload: Readonly<Record<string, unknown>>): boolean {
  return (
    printed(firstOf(asJson(payload), [["tool_response", "interrupted"]], false), "false") === "true"
  );
}
