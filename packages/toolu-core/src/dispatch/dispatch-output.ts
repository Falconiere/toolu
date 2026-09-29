/**
 * Hook-output reading and merging for the PreToolUse dispatcher (#258), a port
 * of the `jq` calls in `dispatch.sh`. Module stdout is read the way
 * `$(jq -r '<path> // empty' <<<"$result")` reads it, and the merged result is
 * printed the way `jq -n` prints it, so the TypeScript dispatcher's bytes match
 * the bash dispatcher's while modules still run on bash.
 */
import { isJsonObject, type JsonObject } from "../config/config-load.ts";
import { toJqJson } from "../state/state-io.ts";

/** `$(...)`: command substitution strips every trailing newline. */
export function substituted(text: string): string {
  return text.replace(/\n+$/, "");
}

/** The single JSON document in `text`, or undefined where `jq` would error or print nothing. */
export function parseDocument(text: string): unknown {
  try {
    const parsed: unknown = JSON.parse(text);
    return parsed;
  } catch {
    return undefined;
  }
}

/** `jq -r` printing of one value, before the substitution strips newlines. */
function jqRaw(value: unknown): string {
  return typeof value === "string" ? value : toJqJson(value, true);
}

/**
 * `$(jq -r '.<path> // empty' <<<"$result")`: "" when the document is not
 * JSON, a step indexes a non-object, or the value is null or false.
 */
export function readField(doc: unknown, path: readonly string[]): string {
  let value: unknown = doc;
  for (const key of path) {
    if (value === null) return "";
    if (!isJsonObject(value)) return "";
    value = value[key];
  }
  if (value === undefined || value === null || value === false) return "";
  return substituted(jqRaw(value));
}

export type Advisories = { contexts: string[]; messages: string[] };

export function emptyAdvisories(): Advisories {
  return { contexts: [], messages: [] };
}

function addOnce(list: string[], text: string): void {
  if (text !== "" && !list.includes(text)) list.push(text);
}

/** Harvest one result's `additionalContext` and `systemMessage`, each deduped by exact text. */
export function collectAdvisories(into: Advisories, doc: unknown): void {
  addOnce(into.contexts, readField(doc, ["hookSpecificOutput", "additionalContext"]));
  addOnce(into.messages, readField(doc, ["systemMessage"]));
}

/** `jq -n`'s default output: two-space pretty print and a newline. */
function jqPrint(value: unknown): string {
  return `${toJqJson(value, true)}\n`;
}

/** `printf '%s\n' "$result"`. */
export function printed(result: string): string {
  return `${result}\n`;
}

function joined(list: readonly string[]): string {
  return list.join("\n\n");
}

/** Mirrors jq's `+`: string concatenation only; anything else is a jq error. */
function concat(left: unknown, right: string): string | undefined {
  return typeof left === "string" ? left + right : undefined;
}

function enriched(ask: JsonObject, ctx: string, msg: string): JsonObject | undefined {
  const hso = ask.hookSpecificOutput;
  if (!isJsonObject(hso)) return undefined;
  const current = hso.permissionDecisionReason;
  const reason = current === undefined || current === null || current === false ? "" : current;
  const nextReason = ctx === "" ? reason : concat(reason, `\n\n${ctx}`);
  if (nextReason === undefined) return undefined;
  const out: JsonObject = {
    ...ask,
    hookSpecificOutput: { ...hso, permissionDecisionReason: nextReason },
  };
  if (msg === "") return out;
  const sys = ask.systemMessage;
  const base = sys === undefined || sys === null || sys === false ? "" : sys;
  const nextMsg = base === "" ? msg : concat(base, `\n\n${msg}`);
  if (nextMsg === undefined) return undefined;
  out.systemMessage = nextMsg;
  return out;
}

/**
 * The held ask, with every advisory appended to its reason (contexts) and its
 * `systemMessage` (messages). Where the bash `jq` enrichment would fail, the
 * ask is emitted exactly as the module wrote it: losing an advisory is
 * cosmetic, losing the decision would turn a prompt into an allow.
 */
export function finalAsk(askResult: string, advisories: Advisories): string {
  const ask = parseDocument(askResult);
  const out = isJsonObject(ask)
    ? enriched(ask, joined(advisories.contexts), joined(advisories.messages))
    : undefined;
  return out === undefined ? printed(askResult) : jqPrint(out);
}

/** The merged advisory object, or "" when there is nothing to say. */
export function finalAdvisory(advisories: Advisories): string {
  const ctx = joined(advisories.contexts);
  const msg = joined(advisories.messages);
  if (ctx === "" && msg === "") return "";
  const out: JsonObject = {};
  if (ctx !== "") out.hookSpecificOutput = { hookEventName: "PreToolUse", additionalContext: ctx };
  if (msg !== "") out.systemMessage = msg;
  return jqPrint(out);
}
