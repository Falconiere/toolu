//! The verdict's review gate (`reviewGate` in `verdict-review.ts`): the
//! push-review v2 schema applied to the state file, with a closed
//! `reason_code` set.

use toolu_runtime::json::js_number;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::git::branch_slug;

use super::gates::{GateContext, field_or, gate, read_json};
use crate::ledger::jq::{alt, get, number, string, to_str};
use crate::ledger::parse::is_file;
use crate::review_state::{has_accepted_reviewer, output_lines, reviewed_files, sorted_unique};

const EMPTY_BLOB_SHA: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

fn codes(reason_code: Option<&str>, round: Ordered) -> Vec<(String, Ordered)> {
  let code = reason_code.map_or(Ordered::Null, string);
  vec![
    ("reason_code".to_owned(), code),
    ("round".to_owned(), round),
  ]
}

/// `(.review_round // 1) | if type == "number" then . else null end`.
fn round_of(state: &Ordered) -> Ordered {
  let one = number(1.0);
  match get(state, "review_round").map(|round| alt(round, &one)) {
    Ok(round @ Ordered::Number(_)) => round.clone(),
    _ => Ordered::Null,
  }
}

/// `String(Math.floor(round))` when that is a plain integer above 5.
fn over_cap(round: &Ordered) -> Option<String> {
  let Ordered::Number(n) = round else {
    return None;
  };
  let whole = js_number(n.as_f64()?.floor());
  let digits = whole.strip_prefix('-').unwrap_or(&whole);
  let integer = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
  let above = !whole.starts_with('-') && (digits.len() > 1 || digits > "5");
  (integer && above).then_some(whole)
}

/// The schema checks: a v1 state, then missing v2 fields.
fn schema_gate(
  version: &str,
  sha: &str,
  findings: &str,
  file: &str,
  code: &dyn Fn(&str) -> Vec<(String, Ordered)>,
) -> Option<Ordered> {
  if version == "1" {
    let reason = "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file";
    return Some(gate("fail", reason, code("schema-v1")));
  }
  if version != "2" || sha.is_empty() || findings.is_empty() {
    let reason =
      format!("push-review state file is corrupted or missing required v2 fields at {file}");
    return Some(gate("fail", &reason, code("schema")));
  }
  None
}

/// The v2 checks once the state parses: schema, reviewer, round cap, staleness, coverage, findings.
fn state_gate(ctx: &GateContext<'_>, state: &Ordered, file: &str) -> Ordered {
  let round = round_of(state);
  let code = |reason: &str| codes(Some(reason), round.clone());
  let empty = string("");
  let text =
    |key: &str| get(state, key).map_or_else(|_| String::new(), |value| to_str(alt(value, &empty)));
  let (version, findings) = (text("version"), text("findings_count"));
  let sha = field_or(state, "diff_sha", "");
  if let Some(failed) = schema_gate(&version, &sha, &findings, file, &code) {
    return failed;
  }
  if !has_accepted_reviewer(state) {
    return gate(
      "fail",
      "state file lists no accepted reviewer",
      code("reviewer"),
    );
  }
  if let Some(whole) = over_cap(&round) {
    let reason = format!("review loop hit {whole} rounds (max 5) on an unchanged diff");
    return gate("escalate", &reason, code("round-cap"));
  }
  if sha != ctx.cur {
    return gate(
      "fail",
      "diff changed since review (state stale)",
      code("stale-diff"),
    );
  }
  let range = format!("{}...HEAD", ctx.base);
  let changed = output_lines(
    &ctx
      .git(&["diff", "--no-color", &range, "--name-only"])
      .unwrap_or_default(),
  );
  if sorted_unique(&changed) != sorted_unique(&reviewed_files(state)) {
    let reason = "reviewed_files does not match the current diff's changed paths";
    return gate("fail", reason, code("file-coverage"));
  }
  if findings != "0" {
    return gate(
      "fail",
      &format!("code review has open findings ({findings})"),
      code("findings"),
    );
  }
  gate("pass", "review satisfied", code("pass"))
}

/// `vd_gate_review`: the push-review v2 state against the current diff.
pub fn review_gate(ctx: &GateContext<'_>) -> Ordered {
  let none = || codes(None, Ordered::Null);
  if ctx.branch == ctx.base {
    return gate("skip", "current branch is the base branch", none());
  }
  if ctx.cur.is_empty() {
    return gate(
      "skip",
      &format!("could not compute diff against {}", ctx.base),
      none(),
    );
  }
  if ctx.cur == EMPTY_BLOB_SHA {
    let reason = format!(
      "diff against {} is empty; verify intent before pushing",
      ctx.base
    );
    return gate("fail", &reason, codes(Some("empty-diff"), Ordered::Null));
  }
  let dir = ctx.state_dir("push-review", "STATE_DIR");
  let file = format!("{}/{}.json", dir.display(), branch_slug(&ctx.branch));
  if !is_file(std::path::Path::new(&file)) {
    let reason = "no push-review state file; run a reviewer and write the state";
    return gate("fail", reason, codes(Some("no-state"), Ordered::Null));
  }
  match read_json(std::path::Path::new(&file)) {
    None | Some(Ordered::Null | Ordered::Bool(false)) => {
      let reason = format!("push-review state file is unparseable at {file}");
      gate("fail", &reason, codes(Some("schema"), Ordered::Null))
    }
    Some(state) => state_gate(ctx, &state, &file),
  }
}

#[cfg(test)]
#[path = "tests/review_test.rs"]
mod tests;
