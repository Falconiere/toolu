//! The push-review v2 state file as jq reads it (`packages/toolu-core/src/ledger/review-state.ts`):
//! the reviewer allow-list, and the reviewer, coverage and `sort -u` checks the
//! verdict's review gate shares with the push-review gate (#422).

use toolu_runtime::json::ordered::Ordered;

use crate::ledger::jq::{alt, each_optional, get, index_of, raw};

/// The reviewers a push-review state may name.
pub const ACCEPTED_REVIEWERS: [&str; 5] = [
  "code-review",
  "toolu-review:review",
  "code-review:xhigh",
  "review",
  "security-review",
];

/// `any($acc[]; . as $x | $r | index($x) != null)` over `(.reviewers // [])`;
/// a jq error is "no".
pub fn has_accepted_reviewer(state: &Ordered) -> bool {
  let empty = Ordered::Array(Vec::new());
  let Ok(reviewers) = get(state, "reviewers").map(|r| alt(r, &empty)) else {
    return false;
  };
  ACCEPTED_REVIEWERS
    .iter()
    .any(|name| index_of(reviewers, name).is_ok_and(|found| !matches!(found, Ordered::Null)))
}

/// `$(… | sort -u)`: unique lines in byte order, trailing newlines stripped; empty lines count.
pub fn sorted_unique(lines: &[String]) -> String {
  let mut sorted: Vec<&String> = lines.iter().collect();
  sorted.sort();
  sorted.dedup();
  let joined: Vec<&str> = sorted.iter().map(|line| line.as_str()).collect();
  joined.join("\n").trim_end_matches('\n').to_owned()
}

/// Command output as the lines `sort` reads: the final newline ends the last line.
pub fn output_lines(text: &str) -> Vec<String> {
  if text.is_empty() {
    return Vec::new();
  }
  text
    .strip_suffix('\n')
    .unwrap_or(text)
    .split('\n')
    .map(str::to_owned)
    .collect()
}

/// `jq -r '.reviewed_files[]'` lines; a jq error yields nothing.
pub fn reviewed_files(state: &Ordered) -> Vec<String> {
  match get(state, "reviewed_files") {
    Ok(Ordered::Null) | Err(_) => Vec::new(),
    Ok(files) => each_optional(files)
      .into_iter()
      .flat_map(|file| raw(file).split('\n').map(str::to_owned).collect::<Vec<_>>())
      .collect(),
  }
}

#[cfg(test)]
#[path = "tests/review_state_test.rs"]
mod tests;
