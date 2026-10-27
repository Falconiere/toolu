//! `toolu status` at a terminal: the snapshot document as a table on rails, one
//! labelled row per fact, with a glyph on the gate and the push review. Pipes
//! and `--json` keep the JSON document.

use serde_json::Value;
use toolu_runtime::term::{Style, Tone, intro, label_row, outro, paint};

/// The label column's width.
const LABELS: usize = 14;

/// The table for `document`, a `toolu status` snapshot.
pub(super) fn render(document: &Value, style: Style) -> String {
  let text = |key: &str| {
    document
      .get(key)
      .and_then(Value::as_str)
      .unwrap_or_default()
  };
  let count = |key: &str| document.get(key).and_then(Value::as_u64).unwrap_or(0);
  let in_repo = !text("repo_root").is_empty();
  let branch = match text("branch") {
    "" => "—".to_owned(),
    "HEAD" => "HEAD (detached)".to_owned(),
    name => name.to_owned(),
  };
  let branch = if in_repo {
    format!("{branch}  ↑{} ↓{}", count("ahead"), count("behind"))
  } else {
    branch
  };
  let repository = if in_repo {
    text("repo_root").to_owned()
  } else {
    paint(style, Tone::Dim, "not a git repository")
  };
  let tree = &document["working_tree"];
  let changes = |key: &str| tree.get(key).and_then(Value::as_u64).unwrap_or(0);
  let rows = [
    ("repository", repository),
    ("branch", branch),
    ("host", text("host").to_owned()),
    (
      "working tree",
      format!(
        "{} staged · {} unstaged · {} untracked",
        changes("staged"),
        changes("unstaged"),
        changes("untracked")
      ),
    ),
    ("gate", gate(&document["gate"], style)),
    ("push review", review(&document["push_review"], style)),
    ("waivers", waivers(&document["waivers"])),
  ];
  let mut table = intro(style, "toolu status");
  for (label, value) in rows {
    table.push_str(&label_row(style, label, LABELS, &value));
  }
  table.push_str(&outro(style, ""));
  table
}

/// `<glyph> <word>`, then `: <reason>` when there is one.
fn marked(style: Style, (glyph, tone): (&str, Tone), word: &str, reason: &str) -> String {
  let glyph = paint(style, tone, glyph);
  if reason.is_empty() {
    format!("{glyph} {word}")
  } else {
    format!("{glyph} {word}: {reason}")
  }
}

fn gate(gate: &Value, style: Style) -> String {
  let reason = gate["reason"].as_str().unwrap_or_default();
  match gate["status"].as_str().unwrap_or("missing") {
    "passing" => marked(style, ("✔", Tone::Good), "passing", ""),
    "failing" => {
      let files = gate["entries"].as_array().map_or(0, Vec::len);
      let noun = if files == 1 { "file" } else { "files" };
      format!(
        "{} ({files} {noun})",
        marked(style, ("✘", Tone::Bad), "failing", reason)
      )
    }
    "invalid" => marked(style, ("▲", Tone::Warn), "invalid", reason),
    _missing => marked(style, ("○", Tone::Dim), "missing", ""),
  }
}

fn review(review: &Value, style: Style) -> String {
  let reason = review["reason"].as_str().unwrap_or_default();
  match review["state"].as_str().unwrap_or("missing") {
    "recorded" => marked(style, ("✔", Tone::Good), "recorded", ""),
    "invalid" => marked(style, ("▲", Tone::Warn), "invalid", reason),
    _missing => marked(style, ("○", Tone::Dim), "missing", ""),
  }
}

fn waivers(waivers: &Value) -> String {
  let active = !waivers["waiver"].is_null();
  let pending = !waivers["pending"].is_null();
  match (active, pending) {
    (false, false) => "none".to_owned(),
    (true, false) => "waiver active".to_owned(),
    (false, true) => "waiver pending".to_owned(),
    (true, true) => "waiver active, waiver pending".to_owned(),
  }
}

#[cfg(test)]
#[path = "tests/table_test.rs"]
mod tests;
