/**
 * The Claude Code statusline payload, read the way the bash renderer's single
 * `jq -r` program read it: each field is `path // alternative // default`,
 * evaluated in order. Indexing a string, number, boolean or array is a jq
 * error that stops the program, so that field and every later one come back
 * empty; fields already printed keep their values.
 */
import { asObject } from "./json.ts";

export type Payload = {
  model: string;
  effort: string;
  cwd: string;
  ctxSize: string;
  ctxUsed: string;
  ctxPct: string;
};

class JqIndexError extends Error {}

/** jq `.key`: null passes through, an object yields the member or null, anything else errors. */
function index(value: unknown, key: string): unknown {
  if (value === null || value === undefined) return null;
  const members = asObject(value);
  if (members === undefined) throw new JqIndexError(key);
  return Object.hasOwn(members, key) ? members[key] : null;
}

/** What `jq -r` prints for a scalar. Non-canonical number literals (`1.0`, `1e3`) print by value. */
function rawText(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

type Field = { key: keyof Payload; paths: readonly (readonly string[])[]; fallback: string };

const FIELDS: readonly Field[] = [
  { key: "model", paths: [["model", "display_name"]], fallback: "Claude" },
  { key: "effort", paths: [["effort", "level"]], fallback: "" },
  { key: "cwd", paths: [["workspace", "current_dir"], ["cwd"]], fallback: "" },
  { key: "ctxSize", paths: [["context_window", "context_window_size"]], fallback: "0" },
  { key: "ctxUsed", paths: [["context_window", "total_input_tokens"]], fallback: "0" },
  { key: "ctxPct", paths: [["context_window", "used_percentage"]], fallback: "" },
];

/** `a // b // fallback`: the first path whose value is neither null nor false. */
function evaluate(doc: unknown, field: Field): string {
  for (const path of field.paths) {
    const value = path.reduce<unknown>(index, doc);
    if (value !== null && value !== false) return rawText(value);
  }
  return field.fallback;
}

function parse(text: string): { ok: true; doc: unknown } | { ok: false } {
  try {
    return { ok: true, doc: JSON.parse(text) };
  } catch {
    return { ok: false };
  }
}

/** Every field, with the bash defaults applied to the ones jq left empty. */
export function readPayload(stdin: string): Payload {
  const out: Payload = { model: "", effort: "", cwd: "", ctxSize: "", ctxUsed: "", ctxPct: "" };
  const parsed = parse(stdin);
  if (parsed.ok) {
    try {
      for (const field of FIELDS) out[field.key] = evaluate(parsed.doc, field);
    } catch (error: unknown) {
      if (!(error instanceof JqIndexError)) throw error;
    }
  }
  return {
    ...out,
    model: out.model === "" ? "Claude" : out.model,
    ctxSize: out.ctxSize === "" ? "0" : out.ctxSize,
    ctxUsed: out.ctxUsed === "" ? "0" : out.ctxUsed,
  };
}
