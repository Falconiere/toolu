/**
 * Plan and spec document parsing (#256), a port of `plan-ledger-parse.sh`:
 * the machine-readable steps block, inline-bold header fields, and the
 * spec's acceptance-criterion ids. Pure reads; nothing here writes.
 */
import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { MODEL_ALIASES } from "../config/config-read.ts";
import { compareJqStrings } from "../state/state-io.ts";
import {
  alt,
  eachOptional,
  isObject,
  jqEquals,
  parseJson,
  raw,
  toStr,
  type Json,
  type JsonObject,
} from "./ledger-jq.ts";

/** A parsed step: the authored fields plus the defaults every consumer relies on. */
export type PlanStep = JsonObject & {
  id: string;
  title: string;
  check: string;
  ac_refs: Json;
  depends_on: Json;
  paths: Json;
  input: Json;
  model: Json;
};

export type ParseResult =
  | { ok: true; steps: PlanStep[] }
  | { ok: false; exitCode: 1 | 2; message: string };

/** `[[:space:]]` in the C locale. */
const SPACE = "[ \\t\\n\\v\\f\\r]";
const STEPS_HEADING = new RegExp(`^## Steps \\(machine-readable\\)${SPACE}*$`);
const JSON_FENCE = new RegExp(`^\`\`\`json${SPACE}*$`);
const CLOSE_FENCE = new RegExp(`^\`\`\`${SPACE}*$`);
const AC_HEADING = new RegExp(`^## Acceptance criteria${SPACE}*$`);
const AC_ID = /\*\*(AC-[0-9]+):\*\*/;

/** bash `[ -f PATH ]`: an existing regular file, symlinks followed. */
export function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** The file's lines as awk and grep see them, or none when it cannot be read. */
function lines(path: string): string[] {
  let text: string;
  try {
    text = readFileSync(path, "utf8");
  } catch {
    return [];
  }
  const out = text.split("\n");
  if (out.at(-1) === "") out.pop();
  return out;
}

/** The first ```json block under the steps heading, as `$(awk ...)` captures it. */
function stepsBlock(path: string): string {
  const captured: string[] = [];
  let inSteps = false;
  let inBlock = false;
  for (const line of lines(path)) {
    if (STEPS_HEADING.test(line)) {
      inSteps = true;
    } else if (inSteps && !inBlock && JSON_FENCE.test(line)) {
      inBlock = true;
    } else if (inBlock && CLOSE_FENCE.test(line)) {
      break;
    } else if (inBlock) {
      captured.push(line);
    }
  }
  return captured.join("\n").replace(/\n+$/, "");
}

function nonEmptyString(value: Json | undefined): value is string {
  return typeof value === "string" && value.length > 0;
}

type StepFields = JsonObject & { id: string; title: string; check: string };

/** A step object with non-empty string id, title and check. */
function isStep(value: Json): value is StepFields {
  return (
    isObject(value) &&
    nonEmptyString(value.id) &&
    nonEmptyString(value.title) &&
    nonEmptyString(value.check)
  );
}

/** `id=model` for each step whose `model` is present but not a routable alias. */
function badModels(steps: JsonObject[]): string[] {
  return steps
    .filter((step) => (step.model ?? null) !== null)
    .filter((step) => {
      const model = step.model ?? null;
      return typeof model !== "string" || !MODEL_ALIASES.some((alias) => alias === model);
    })
    .map((step) => `${toStr(step.id ?? null)}=${toStr(step.model ?? null)}`);
}

/** Ids of steps whose `paths` is present but not an array of non-empty strings. */
function badPaths(steps: JsonObject[]): string[] {
  return steps
    .filter((step) => (step.paths ?? null) !== null)
    .filter((step) => {
      const paths = step.paths ?? null;
      return !Array.isArray(paths) || paths.some((p) => typeof p !== "string" || p === "");
    })
    .map((step) => toStr(step.id ?? null));
}

/**
 * The optional authored fields backfilled with jq `//` defaults, in bash's key
 * order: existing keys keep their place, and missing ones are appended.
 */
function normalize(step: StepFields): PlanStep {
  return {
    ...step,
    ac_refs: alt(step.ac_refs, []),
    depends_on: alt(step.depends_on, []),
    paths: alt(step.paths, []),
    input: alt(step.input, null),
    model: alt(step.model, null),
  };
}

function fail(exitCode: 1 | 2, message: string): ParseResult {
  return { ok: false, exitCode, message: `plan-ledger-parse: ${message}` };
}

const TYPE_RANK = ["null", "boolean", "number", "string", "array", "object"];

/** jq's order between values of different types. */
function rank(value: Json): number {
  return TYPE_RANK.indexOf(value === null ? "null" : Array.isArray(value) ? "array" : typeof value);
}

/**
 * `pl_parse_steps DOC`: the validated, normalized steps array. `doc` is
 * resolved against `cwd` but echoed as given, as bash prints it. A bad
 * `paths` field fails with exit 2, and every other failure with exit 1.
 */
export function parseSteps(doc: string, options: { cwd?: string } = {}): ParseResult {
  const path = resolve(options.cwd ?? process.cwd(), doc);
  if (!isFile(path)) return fail(1, `plan doc not found: ${doc}`);
  const block = stepsBlock(path);
  if (block === "") return fail(1, `no '## Steps (machine-readable)' json block in ${doc}`);
  const value = parseJson(block);
  if (!Array.isArray(value) || value.length === 0 || !value.every(isStep)) {
    return fail(1, `steps block in ${doc} is not a non-empty array of {id,title,check} strings`);
  }
  const steps = value;
  const models = badModels(steps);
  if (models.length > 0) {
    return fail(
      1,
      `invalid step model in ${doc} (${models.join(", ")}); allowed: ${MODEL_ALIASES.join(" ")}`,
    );
  }
  const paths = badPaths(steps);
  if (paths.length > 0) {
    return fail(
      2,
      `invalid step paths in ${doc} (${paths.join(", ")}); expected an array of non-empty strings`,
    );
  }
  return { ok: true, steps: steps.map(normalize) };
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * `pl_doc_field DOC FIELD`: the trimmed value of `**FIELD:**` on the first
 * line holding it, cut at the next whitespace-preceded `**Key:**`. Empty when
 * the doc or the field is absent.
 */
export function docField(doc: string, field: string): string {
  if (doc === "" || field === "" || !isFile(doc)) return "";
  const key = `**${field}:**`;
  const line = lines(doc).find((l) => l.includes(key));
  if (line === undefined) return "";
  return line
    .replace(new RegExp(`^[\\s\\S]*${escapeRegExp(key)}${SPACE}*`), "")
    .replace(new RegExp(`${SPACE}+\\*\\*[^*]+:\\*\\*[\\s\\S]*$`), "")
    .replace(new RegExp(`^${SPACE}+`), "")
    .replace(new RegExp(`${SPACE}+$`), "");
}

/** `pl_parse_acs SPEC`: `AC-<n>` ids under `## Acceptance criteria`, deduped in document order. */
export function parseAcs(doc: string): string[] {
  if (doc === "" || !isFile(doc)) return [];
  const ids: string[] = [];
  let inAc = false;
  for (const line of lines(doc)) {
    if (AC_HEADING.test(line)) {
      inAc = true;
      continue;
    }
    if (inAc && line.startsWith("## ")) inAc = false;
    const id = inAc ? AC_ID.exec(line)?.[1] : undefined;
    if (id !== undefined && !ids.includes(id)) ids.push(id);
  }
  return ids;
}

/** bash `tr '[:upper:]' '[:lower:]'` then `""|none`: the spec-less marker. */
export function isSpecless(spec: string): boolean {
  const lower = spec.replace(/[A-Z]/g, (c) => c.toLowerCase());
  return lower === "" || lower === "none";
}

/** jq `unique`: sorted by jq's value order, duplicates dropped. */
function jqUnique(values: Json[]): Json[] {
  const sorted = values.toSorted((a, b) => {
    const byType = rank(a) - rank(b);
    if (byType !== 0) return byType;
    if (typeof a === "number" && typeof b === "number") return a - b;
    if (typeof a === "string" && typeof b === "string") return compareJqStrings(a, b);
    if (typeof a === "boolean" && typeof b === "boolean") return Number(a) - Number(b);
    return compareJqStrings(JSON.stringify(a), JSON.stringify(b));
  });
  return sorted.filter((v, i) => i === 0 || !jqEquals(v, sorted[i - 1] ?? null));
}

export type AcRefsResult = { ok: boolean; dangling: string[]; message?: string };

/**
 * `pl_check_ac_refs PLAN [SPEC]`: every `ac_refs` id the spec does not
 * declare, in C-locale sort order. A spec-less plan passes, and a plan that
 * does not parse fails with the parse message.
 */
export function checkAcRefs(plan: string, spec = "", options: { cwd?: string } = {}): AcRefsResult {
  if (isSpecless(spec)) return { ok: true, dangling: [] };
  const parsed = parseSteps(plan, options);
  if (!parsed.ok) return { ok: false, dangling: [], message: parsed.message };
  // `refs=$(jq -r '... | unique[]')`: one line per rendering, trailing newlines dropped.
  const refsText = jqUnique(parsed.steps.flatMap((step) => eachOptional(step.ac_refs)))
    .map(raw)
    .join("\n")
    .replace(/\n+$/, "");
  if (refsText === "") return { ok: true, dangling: [] };
  const refs = refsText.split("\n");
  const declared = new Set(parseAcs(resolve(options.cwd ?? process.cwd(), spec)));
  const dangling = [...new Set(refs.filter((ref) => !declared.has(ref)))].toSorted(
    compareJqStrings,
  );
  return { ok: dangling.length === 0, dangling };
}
