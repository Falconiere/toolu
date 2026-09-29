/**
 * The push-review v2 state file as jq reads it (#256, #262): the reviewer
 * allow-list and the reviewer, coverage and `sort -u` checks shared by the
 * push-review gate and `verdict.sh`'s review gate.
 */
import { compareJqStrings } from "../state/state-io.ts";
import { JqError, alt, eachOptional, get, jqIndex, raw, type Json } from "./ledger-jq.ts";

/** Reviewer allow-list: the literal `push-review.sh`'s `accepted_reviewers` held. */
export const ACCEPTED_REVIEWERS: readonly string[] = [
  "code-review",
  "toolu-review:review",
  "code-review:xhigh",
  "review",
  "security-review",
];

/** `any($acc[]; . as $x | $r | index($x) != null)` over `(.reviewers // [])`; a jq error is "no". */
export function hasAcceptedReviewer(state: Json): boolean {
  try {
    const reviewers = alt(get(state, "reviewers"), []);
    return ACCEPTED_REVIEWERS.some((name) => jqIndex(reviewers, name) !== null);
  } catch (error) {
    if (error instanceof JqError) return false;
    throw error;
  }
}

/** `$(... | sort -u)`: unique lines in byte order, trailing newlines stripped. Empty lines count. */
export function sortedUnique(lines: string[]): string {
  return [...new Set(lines)].toSorted(compareJqStrings).join("\n").replace(/\n+$/, "");
}

/** Command output as the lines `sort` reads: the final newline ends the last line. */
export function outputLines(text: string): string[] {
  return text === "" ? [] : text.replace(/\n$/, "").split("\n");
}

/** `jq -r '.reviewed_files[]'` lines; a jq error yields what was printed before it (nothing). */
export function reviewedFiles(state: Json): string[] {
  try {
    const files = get(state, "reviewed_files");
    if (files === null) throw new JqError("Cannot iterate over null");
    return eachOptional(files).flatMap((file) => raw(file).split("\n"));
  } catch (error) {
    if (error instanceof JqError) return [];
    throw error;
  }
}
