/**
 * Plan-doc parsing: the machine-readable steps block and the `**Issue:**`
 * header key. The grammar mirrors toolu's plan-ledger parser so a jira plan
 * doc stays readable by the same tooling: an exact `## Steps
 * (machine-readable)` heading followed by the first ```json fence.
 */
import { CliExit } from "@toolu/core/cli";
import { readFileSync, statSync } from "node:fs";
import { isObject, type JsonObject } from "./jq.ts";

/** A Jira key: LETTERS-DIGITS. Anything else could steer a ledger filename. */
export const KEY_PATTERN = /^[A-Za-z][A-Za-z0-9_]*-[0-9]+$/;

export interface Step extends JsonObject {
  readonly id: string;
  readonly title: string;
  readonly check: string;
  readonly activity: unknown;
}

function isFile(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isFile() === true;
}

/** The lines inside the first ```json fence after the steps heading, trailing newlines dropped. */
function stepsBlock(content: string): string {
  const lines: string[] = [];
  let inSteps = false;
  let inBlock = false;
  for (const line of content.split("\n")) {
    // awk checked the heading first on every line, so a heading is never block content.
    if (/^## Steps \(machine-readable\)\s*$/.test(line)) {
      inSteps = true;
    } else if (inBlock && /^```\s*$/.test(line)) {
      break;
    } else if (inBlock) {
      lines.push(line);
    } else if (inSteps && /^```json\s*$/.test(line)) {
      inBlock = true;
    }
  }
  return lines.join("\n").replace(/\n+$/, "");
}

function nonEmptyString(value: unknown): boolean {
  return typeof value === "string" && value.length > 0;
}

function isStep(value: unknown): value is JsonObject {
  return (
    isObject(value) &&
    nonEmptyString(value["id"]) &&
    nonEmptyString(value["title"]) &&
    nonEmptyString(value["check"])
  );
}

function parsed(block: string): unknown {
  try {
    return JSON.parse(block);
  } catch {
    return undefined;
  }
}

/**
 * The validated steps of `doc`: a non-empty array of `{id,title,check}` with
 * non-empty strings, each with `activity` backfilled to null (false too, as
 * jq's `//` did). Every problem exits 1 with bash's message.
 */
export function parseSteps(doc: string): Step[] {
  if (!isFile(doc)) throw new CliExit(1, `jira plan: plan doc not found: ${doc}`);
  const block = stepsBlock(readFileSync(doc, "utf8"));
  if (block === "") {
    throw new CliExit(1, `jira plan: no '## Steps (machine-readable)' json block in ${doc}`);
  }
  const steps = parsed(block);
  if (!Array.isArray(steps) || steps.length === 0 || !steps.every(isStep)) {
    throw new CliExit(
      1,
      `jira plan: steps block in ${doc} is not a non-empty array of {id,title,check} strings`,
    );
  }
  return steps.map((step) => ({
    ...step,
    id: String(step["id"]),
    title: String(step["title"]),
    check: String(step["check"]),
    activity:
      step["activity"] === undefined || step["activity"] === false ? null : step["activity"],
  }));
}

function escaped(text: string): string {
  return text.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * The trimmed value of an inline-bold `**<field>:**` on the first line that
 * has one, stopping at the next `**Other:**`; empty when absent. Never fails.
 */
export function docField(doc: string, field: string): string {
  if (doc === "" || field === "" || !isFile(doc)) return "";
  const marker = `**${field}:**`;
  const line = readFileSync(doc, "utf8")
    .split("\n")
    .find((candidate) => candidate.includes(marker));
  if (line === undefined) return "";
  return line
    .replace(new RegExp(`.*\\*\\*${escaped(field)}:\\*\\*\\s*`), "")
    .replace(/\s+\*\*[^*]+:\*\*.*$/, "")
    .trim();
}

/** The key from the doc's `**Issue:** <KEY>` header, or exit 1 when it is not a Jira key. */
export function issueKey(doc: string): string {
  const key = docField(doc, "Issue");
  if (!KEY_PATTERN.test(key)) {
    throw new CliExit(1, `jira plan: doc is missing a valid '**Issue:** <KEY>' header: ${doc}`);
  }
  return key;
}
