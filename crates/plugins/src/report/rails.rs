//! The terminal form of a verb's report: a section per host, a glyph per
//! plugin, and one closing line with the counts and the time taken. The plain
//! form in `report.rs` stays what pipes and scripts read.

use std::time::Duration;

use toolu_runtime::term::{Style, Tone, intro, outro, paint, row, section};

use crate::host::Target;
use crate::report::Row;
use crate::step::{Status, Step};

/// How a report is framed: its style, the time the run took, and the title of
/// the intro when the terminal did not already show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
  /// Mono or Color.
  pub(crate) style: Style,
  /// The time the run took.
  pub(crate) elapsed: Duration,
  /// `toolu plugins <verb>`, when the report itself opens with it.
  pub(crate) title: Option<String>,
}

/// Every status, in the order the closing line counts them.
const ORDER: [Status; 8] = [
  Status::Installed,
  Status::Updated,
  Status::Removed,
  Status::Already,
  Status::Current,
  Status::Skew,
  Status::Failed,
  Status::Skipped,
];

fn mark(status: Status) -> (&'static str, Tone) {
  match status {
    Status::Installed | Status::Updated | Status::Removed => ("✔", Tone::Good),
    Status::Already | Status::Current => ("●", Tone::Dim),
    Status::Skew => ("▲", Tone::Warn),
    Status::Failed => ("✘", Tone::Bad),
    Status::Skipped => ("○", Tone::Dim),
  }
}

fn open(frame: &Frame) -> String {
  frame
    .title
    .as_deref()
    .map(|title| intro(frame.style, title))
    .unwrap_or_default()
}

fn close(frame: &Frame, counts: &str) -> String {
  let secs = frame.elapsed.as_secs_f64();
  outro(frame.style, &format!("{counts}  ({secs:.1}s)"))
}

/// The steps of each host, the hosts left out with why, then the counts of
/// every status that occurred.
pub(crate) fn steps(
  frame: &Frame,
  sections: &[(Target, Vec<Step>)],
  notes: &[(Target, String)],
  dry_run: bool,
) -> String {
  let width = sections
    .iter()
    .flat_map(|(_, steps)| steps.iter().map(|step| step.name.chars().count()))
    .max()
    .unwrap_or(0);
  let mut text = open(frame);
  for (index, (target, steps)) in sections.iter().enumerate() {
    if index > 0 {
      text.push_str(&gap(frame.style));
    }
    text.push_str(&section(frame.style, target.name()));
    for step in steps {
      let detail = if dry_run && step.status == Status::Skipped {
        format!("would run: {}", step.argv.join(" "))
      } else {
        step.detail.clone()
      };
      text.push_str(&row(
        frame.style,
        mark(step.status),
        &step.name,
        width,
        &detail,
      ));
    }
  }
  for (index, (target, note)) in notes.iter().enumerate() {
    if index > 0 || !sections.is_empty() {
      text.push_str(&gap(frame.style));
    }
    text.push_str(&section(frame.style, target.name()));
    text.push_str(&row(frame.style, ("○", Tone::Dim), note, 0, ""));
  }
  text.push_str(&close(frame, &counts(sections)));
  text
}

/// `<n> <status>` for every status that occurred, in `ORDER`.
fn counts(sections: &[(Target, Vec<Step>)]) -> String {
  let counts: Vec<String> = ORDER
    .iter()
    .filter_map(|status| {
      let count = sections
        .iter()
        .flat_map(|(_, steps)| steps)
        .filter(|step| step.status == *status)
        .count();
      (count > 0).then(|| format!("{count} {}", status.name()))
    })
    .collect();
  if counts.is_empty() {
    "nothing to do".to_owned()
  } else {
    counts.join(" · ")
  }
}

/// The catalog joined with one host's listing.
pub(crate) fn list(frame: &Frame, target: Target, rows: &[Row]) -> String {
  let width = rows
    .iter()
    .map(|row| row.name.chars().count())
    .max()
    .unwrap_or(0);
  let mut text = open(frame);
  text.push_str(&section(frame.style, target.name()));
  for item in rows {
    let version = item.version.as_deref().unwrap_or("?");
    let (glyph, detail) = match (item.installed, item.enabled) {
      (false, _) => (("○", Tone::Dim), "not installed".to_owned()),
      (true, true) => (("✔", Tone::Good), version.to_owned()),
      (true, false) => (("▲", Tone::Warn), format!("{version} disabled")),
    };
    text.push_str(&row(frame.style, glyph, &item.name, width, &detail));
  }
  let installed = rows.iter().filter(|row| row.installed).count();
  let missing = rows.len().saturating_sub(installed);
  text.push_str(&close(
    frame,
    &format!("{installed} installed · {missing} not installed"),
  ));
  text
}

/// The empty rail between two host sections.
fn gap(style: Style) -> String {
  format!("{}\n", paint(style, Tone::Dim, "│"))
}

#[cfg(test)]
#[path = "tests/rails_test.rs"]
mod tests;
