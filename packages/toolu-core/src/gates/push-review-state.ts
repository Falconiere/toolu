/**
 * push-review's state-file checks (#262), in `push-review.sh`'s order: schema
 * v1, schema, reviewer, round cap, stale diff, open findings, file coverage.
 * Each field is read the way `jq -r '<path> // ""'` inside `$(...)` read it,
 * so a hand-edited file passes or fails exactly where bash did.
 */
import { alt, get, JqError, raw, type Json } from "../ledger/ledger-jq.ts";
import {
  ACCEPTED_REVIEWERS,
  hasAcceptedReviewer,
  outputLines,
  reviewedFiles,
  sortedUnique,
} from "../ledger/review-state.ts";

/** A failed check: its telemetry reason code, the reason shown, and the state's round. */
export type ReviewFailure = { code: string; reason: string; round: string };

/** `$(jq -r 'EXPR' FILE 2>/dev/null || echo FALLBACK)`: a jq error prints the fallback. */
function field(doc: Json, key: string, dflt: Json, fallback: string): string {
  try {
    return raw(alt(get(doc, key), dflt)).replace(/\n+$/, "");
  } catch (error) {
    if (error instanceof JqError) return fallback;
    throw error;
  }
}

/**
 * `[[ $round =~ ^[0-9]+$ ]] && (( round > 5 ))`: bash arithmetic reads a
 * leading 0 as octal (an 8 or 9 then fails the test) and wraps at 64 bits.
 */
function overCap(round: string): boolean {
  if (!/^[0-9]+$/.test(round)) return false;
  const octal = round.length > 1 && round.startsWith("0");
  if (octal && /[89]/.test(round)) return false;
  return BigInt.asIntN(64, BigInt(octal ? `0o${round}` : round)) > 5n;
}

/** `comm -23 <(printf '%s\n' "$a") <(printf '%s\n' "$b")` over two `sort -u` outputs. */
function only(a: string, b: string): string {
  const other = new Set(b.split("\n"));
  return a
    .split("\n")
    .filter((line) => !other.has(line))
    .join("\n")
    .replace(/\n+$/, "");
}

/** `jq -r '.review_round // 1'`, where a jq error prints `1`. */
export function stateRound(doc: Json): string {
  return field(doc, "review_round", 1, "1");
}

const REVIEWER_LIST = JSON.stringify(ACCEPTED_REVIEWERS);
const HINT =
  'the built-in `/code-review xhigh --fix` skill, recorded as "code-review" (or the `toolu-review:review` skill)';

function fail(code: string, reason: string, round = ""): ReviewFailure {
  return { code, reason, round };
}

/** `reviewed_files` against the diff's changed paths, both `sort -u`ed as bash compared them. */
function coverageFailure(
  doc: Json,
  file: string,
  base: string,
  changed: string,
  round: string,
): ReviewFailure | undefined {
  const changedSorted = sortedUnique(outputLines(changed));
  const reviewedSorted = sortedUnique(reviewedFiles(doc));
  if (changedSorted === reviewedSorted) return undefined;
  const missing = only(changedSorted, reviewedSorted);
  const extra = only(reviewedSorted, changedSorted);
  let reason = `reviewed_files does not match the current diff at ${file}.`;
  if (missing !== "")
    reason += `\nMissing from reviewed_files (changed but not reviewed): ${missing}`;
  if (extra !== "") reason += `\nIn reviewed_files but not in the current diff: ${extra}`;
  reason += `\nRe-review the full diff and rewrite reviewed_files to match \`git diff ${base}...HEAD --name-only\` exactly.`;
  return fail("file-coverage", reason, round);
}

/** The first check `doc` (the parsed state file, `null` when unreadable) fails, or undefined. */
export function stateFailure(
  doc: Json,
  file: string,
  cur: string,
  base: string,
  changed: string,
): ReviewFailure | undefined {
  const version = field(doc, "version", "", "");
  const sha = field(doc, "diff_sha", "", "");
  const findings = field(doc, "findings_count", "", "");
  if (version === "1") {
    return fail(
      "schema-v1",
      "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file",
    );
  }
  if (version !== "2" || sha === "" || findings === "") {
    return fail("schema", `state file corrupted at ${file}; delete and re-review`);
  }
  const round = stateRound(doc);
  if (!hasAcceptedReviewer(doc)) {
    return fail(
      "reviewer",
      `state file lists no accepted reviewer at ${file}\n\`reviewers\` must include at least one of: ${REVIEWER_LIST}\nRun a reviewer — use ${HINT} — then rewrite the state file.`,
      round,
    );
  }
  if (overCap(round)) {
    return fail(
      "round-cap",
      `ESCALATE: review loop hit ${round} rounds (max 5) on an unchanged diff at ${file}. Reviewers keep finding new issues after each fix — stop auto-looping and surface the current findings to the human. Babysit: treat as Escalation stop (Step 6).`,
      round,
    );
  }
  if (sha !== cur) {
    return fail(
      "stale-diff",
      `Code review required: diff changed since review.\nCurrent diff SHA: ${cur}\nState file: ${file} (stale)\nRe-run reviewers on the new diff and rewrite the state file.`,
      round,
    );
  }
  if (findings !== "0") {
    return fail(
      "findings",
      `Code review has open findings (${findings}).\nState file: ${file}\nAddress every finding (any finding blocks). Re-commit. Re-run reviewers. Rewrite state file with findings_count=0.`,
      round,
    );
  }
  return coverageFailure(doc, file, base, changed, round);
}
