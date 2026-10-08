//! Push-review's state-file checks, in the original gate's order.

use std::fmt::Write as _;

use toolu_runtime::json::ordered::Ordered;

use crate::ledger::jq::{alt, get, raw, string};
use crate::review_state::{has_accepted_reviewer, output_lines, reviewed_files, sorted_unique};

/// One failed review-state check.
pub(crate) struct ReviewFailure {
  pub(crate) code: &'static str,
  pub(crate) reason: String,
  pub(crate) round: String,
}

struct Review<'a> {
  doc: &'a Ordered,
  file: &'a str,
  current: &'a str,
  base: &'a str,
  changed: &'a str,
  sha: &'a str,
  findings: &'a str,
}

const REVIEWER_LIST: &str =
  "[\"code-review\",\"toolu-review:review\",\"code-review:xhigh\",\"review\",\"security-review\"]";
const HINT: &str = "the built-in `/code-review xhigh --fix` skill, recorded as \"code-review\" (or the `toolu-review:review` skill)";

fn field(doc: &Ordered, key: &str, default: &Ordered, fallback: &str) -> String {
  get(doc, key).map_or_else(
    |_| fallback.to_owned(),
    |value| raw(alt(value, default)).trim_end_matches('\n').to_owned(),
  )
}

/// `jq -r '.review_round // 1'`, with `1` on a jq error.
pub(crate) fn state_round(doc: &Ordered) -> String {
  field(doc, "review_round", &Ordered::Number(1.into()), "1")
}

fn fail(code: &'static str, reason: String, round: &str) -> ReviewFailure {
  ReviewFailure {
    code,
    reason,
    round: round.to_owned(),
  }
}

/// Bash arithmetic parses a leading zero as octal and wraps at signed 64 bits.
fn over_cap(round: &str) -> bool {
  if round.is_empty() || !round.bytes().all(|byte| byte.is_ascii_digit()) {
    return false;
  }
  let octal = round.len() > 1 && round.starts_with('0');
  let radix: u64 = if octal { 8 } else { 10 };
  let mut value = 0_u64;
  for byte in round.bytes() {
    let digit = u64::from(byte - b'0');
    if digit >= radix {
      return false;
    }
    value = value.wrapping_mul(radix).wrapping_add(digit);
  }
  value.cast_signed() > 5
}

fn only(a: &str, b: &str) -> String {
  let other: std::collections::BTreeSet<&str> = b.split('\n').collect();
  a.split('\n')
    .filter(|line| !other.contains(line))
    .collect::<Vec<_>>()
    .join("\n")
    .trim_end_matches('\n')
    .to_owned()
}

fn coverage_failure(
  doc: &Ordered,
  file: &str,
  base: &str,
  changed: &str,
  round: &str,
) -> Option<ReviewFailure> {
  let changed = sorted_unique(&output_lines(changed));
  let reviewed = sorted_unique(&reviewed_files(doc));
  if changed == reviewed {
    return None;
  }
  let missing = only(&changed, &reviewed);
  let extra = only(&reviewed, &changed);
  let mut reason = format!("reviewed_files does not match the current diff at {file}.");
  if !missing.is_empty() {
    let _ = write!(
      reason,
      "\nMissing from reviewed_files (changed but not reviewed): {missing}"
    );
  }
  if !extra.is_empty() {
    let _ = write!(
      reason,
      "\nIn reviewed_files but not in the current diff: {extra}"
    );
  }
  let _ = write!(
    reason,
    "\nRe-review the full diff and rewrite reviewed_files to match `git diff {base}...HEAD --name-only` exactly."
  );
  Some(fail("file-coverage", reason, round))
}

fn review_failure(review: &Review<'_>) -> Option<ReviewFailure> {
  let Review {
    doc,
    file,
    current,
    base,
    changed,
    sha,
    findings,
  } = *review;
  let round = state_round(doc);
  if !has_accepted_reviewer(doc) {
    return Some(fail(
      "reviewer",
      format!(
        "state file lists no accepted reviewer at {file}\n`reviewers` must include at least one of: {REVIEWER_LIST}\nRun a reviewer — use {HINT} — then rewrite the state file."
      ),
      &round,
    ));
  }
  if over_cap(&round) {
    return Some(fail(
      "round-cap",
      format!(
        "ESCALATE: review loop hit {round} rounds (max 5) on an unchanged diff at {file}. Reviewers keep finding new issues after each fix — stop auto-looping and surface the current findings to the human. Babysit: treat as Escalation stop (Step 6)."
      ),
      &round,
    ));
  }
  if sha != current {
    return Some(fail(
      "stale-diff",
      format!(
        "Code review required: diff changed since review.\nCurrent diff SHA: {current}\nState file: {file} (stale)\nRe-run reviewers on the new diff and rewrite the state file."
      ),
      &round,
    ));
  }
  if findings != "0" {
    return Some(fail(
      "findings",
      format!(
        "Code review has open findings ({findings}).\nState file: {file}\nAddress every finding (any finding blocks). Re-commit. Re-run reviewers. Rewrite state file with findings_count=0."
      ),
      &round,
    ));
  }
  coverage_failure(doc, file, base, changed, &round)
}

/// First state check that fails, or none for a clean v2 review.
pub(crate) fn state_failure(
  doc: &Ordered,
  file: &str,
  current: &str,
  base: &str,
  changed: &str,
) -> Option<ReviewFailure> {
  let empty = string("");
  let version = field(doc, "version", &empty, "");
  let sha = field(doc, "diff_sha", &empty, "");
  let findings = field(doc, "findings_count", &empty, "");
  if version == "1" {
    return Some(fail(
      "schema-v1",
      "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file".to_owned(),
      "",
    ));
  }
  if version != "2" || sha.is_empty() || findings.is_empty() {
    return Some(fail(
      "schema",
      format!("state file corrupted at {file}; delete and re-review"),
      "",
    ));
  }
  review_failure(&Review {
    doc,
    file,
    current,
    base,
    changed,
    sha: &sha,
    findings: &findings,
  })
}

#[cfg(test)]
#[path = "tests/push_review_state_test.rs"]
mod tests;
