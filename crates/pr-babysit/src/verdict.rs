//! The CI review bot's comment, parsed: whether it is a review, its checklist
//! state, the verdict, the findings with their `path:line:<8 hex>` keys, and
//! the Top-N must-fix lines.

use serde_json::{Value, json};

use crate::pattern::{self, captures, group, is};

pattern::pattern!(
  REVIEW_MARKER,
  r"^### Code Review|^### PR Review in Progress|\[View job\]\([^)]*actions/runs/[0-9]+|`agent-merge-[a-z]|^`(?:merge-approved|request-changes)`$"
);
pattern::pattern!(UNCHECKED, r"^\s*-\s*\[\s*\]");
pattern::pattern!(CHECKED, r"^\s*-\s*\[[xX]\]");
pattern::pattern!(LABEL_ITEM, r"Set verdict label \(`([^`]+)`\)");
pattern::pattern!(
  LABEL_CHIP,
  r"^`(agent-merge-[a-z-]+|merge-approved|request-changes)`$"
);
pattern::pattern!(LABEL_ANY, r"agent-merge-[a-z-]+");
pattern::pattern!(VERDICT_LINE, r"^\*\*Verdict:\*\*");
pattern::pattern!(CHANGES_REQUESTED, r"(?i)changes requested");
pattern::pattern!(APPROVED, r"(?i)approved");
pattern::pattern!(
  CHANGES_BOLD,
  r"(?i)\*\*Changes requested\*\*|changes-requested"
);
pattern::pattern!(APPROVED_BOLD, r"(?i)\*\*Approved\*\*");
pattern::pattern!(PROVIDER, r"(?i)review incomplete|provider error");
pattern::pattern!(FINDINGS_HEAD, r"^### Findings(?:\s|$)");
pattern::pattern!(ANY_HEAD, r"^### ");
pattern::pattern!(FINDING, r"^`([^`]+)`: (blocker|high|medium|low|nit): (.*)$");
pattern::pattern!(PATH_LINE, r"^(.+):([0-9]+)$");
pattern::pattern!(TOP_N_HEAD, r"(?i)^###\s+Top-N\s+must-fix(?:\s|$)");
pattern::pattern!(TOP_N_END, r"^### |^<details>");
pattern::pattern!(BULLET, r"^[-*] ");
pattern::pattern!(NUMBERED, r"^[0-9]+\.\s+");

fn not_a_review() -> Value {
  json!({
    "is_review_comment": false, "state": "unknown", "complete": false,
    "verdict": "none", "verdict_label": "", "findings": [],
  })
}

/// Parse one review-bot comment body.
pub(crate) fn parse(input: &str) -> Value {
  if input.trim().is_empty() {
    return not_a_review();
  }
  let lines: Vec<&str> = input.split('\n').collect();
  if !lines.iter().any(|line| is(&REVIEW_MARKER, line)) {
    return not_a_review();
  }
  let unchecked = lines.iter().any(|line| is(&UNCHECKED, line));
  let checked = lines.iter().any(|line| is(&CHECKED, line));
  let mut state = match (checked, unchecked) {
    (false, false) => "unknown",
    (_, true) => "in_progress",
    (true, false) => "complete",
  };
  let label = verdict_label(input, &lines);
  let verdict = verdict_of(&label, input, &lines);
  let verdict_line = lines
    .iter()
    .find(|line| is(&VERDICT_LINE, line))
    .copied()
    .unwrap_or("");
  if is(&PROVIDER, verdict_line) {
    state = "provider_error";
  }
  json!({
    "is_review_comment": true,
    "state": state,
    "complete": state == "complete",
    "verdict": verdict,
    "verdict_label": label,
    "findings": findings(&lines),
    "must_fix": must_fix(&lines),
  })
}

fn verdict_label(input: &str, lines: &[&str]) -> String {
  if let Some(label) = group(&LABEL_ITEM, input, 1) {
    return label;
  }
  let chip = lines
    .iter()
    .filter_map(|line| group(&LABEL_CHIP, line, 1))
    .next_back();
  chip
    .or_else(|| group(&LABEL_ANY, input, 0))
    .unwrap_or_default()
}

fn verdict_of(label: &str, input: &str, lines: &[&str]) -> &'static str {
  if label.contains("approved") {
    return "approved";
  }
  if label.contains("blocked") || label.contains("changes") {
    return "changes";
  }
  let summary = lines
    .iter()
    .find(|line| is(&VERDICT_LINE, line))
    .copied()
    .unwrap_or("");
  if is(&CHANGES_REQUESTED, summary) {
    "changes"
  } else if is(&APPROVED, summary) {
    "approved"
  } else if is(&CHANGES_BOLD, input) {
    "changes"
  } else if is(&APPROVED_BOLD, input) {
    "approved"
  } else {
    "none"
  }
}

fn findings(lines: &[&str]) -> Vec<Value> {
  let mut found = Vec::new();
  let mut inside = false;
  for line in lines {
    if is(&FINDINGS_HEAD, line) {
      inside = true;
      continue;
    }
    if is(&ANY_HEAD, line) {
      inside = false;
    }
    if !inside {
      continue;
    }
    if let Some(caps) = captures(&FINDING, line) {
      let field = |at: usize| caps.get(at).map_or("", |found| found.as_str());
      found.push(finding(field(1), field(2), field(3)));
    }
  }
  found
}

fn finding(raw_path: &str, severity: &str, text: &str) -> Value {
  let (path, line_text) = captures(&PATH_LINE, raw_path).map_or((raw_path, ""), |caps| {
    (
      caps.get(1).map_or(raw_path, |found| found.as_str()),
      caps.get(2).map_or("", |found| found.as_str()),
    )
  });
  let hash = crate::sha1::hex(text.as_bytes());
  let short = hash.get(..8).unwrap_or(&hash);
  json!({
    "path": path,
    "line": line_text.parse::<u64>().ok(),
    "severity": severity,
    "text": text,
    "key": format!("{path}:{line_text}:{short}"),
  })
}

fn must_fix(lines: &[&str]) -> Vec<String> {
  let mut items = Vec::new();
  let mut inside = false;
  for line in lines {
    if is(&TOP_N_HEAD, line) {
      inside = true;
      continue;
    }
    if is(&TOP_N_END, line) {
      inside = false;
    }
    let item = line.trim();
    if !inside || item.is_empty() {
      continue;
    }
    let item = strip(&NUMBERED, &strip(&BULLET, item));
    if !item.is_empty() {
      items.push(item);
    }
  }
  items
}

fn strip(pattern: &pattern::Pattern, text: &str) -> String {
  pattern
    .as_ref()
    .map_or_else(|| text.to_owned(), |re| re.replace(text, "").into_owned())
}

#[cfg(test)]
#[path = "tests/verdict_test.rs"]
mod tests;
