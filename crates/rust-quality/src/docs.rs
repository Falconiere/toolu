//! The docs advisory, the port of 90-docs: public items in non-test src/ files
//! with no `///` doc, and doc blocks over 12 lines. It never touches the gate.

use std::sync::LazyLock;

use crate::file::{RsFile, Rx, hit, rx};

static BLANK: Rx = LazyLock::new(|| rx(r"^[[:space:]]*$"));
static ATTRIBUTE: Rx = LazyLock::new(|| rx(r"^[[:space:]]*#\["));
static PUBLIC_ITEM: Rx = LazyLock::new(|| {
  rx(
    r"^[[:space:]]*pub(\([^)]+\))?[[:space:]]+(fn|struct|enum|trait|const|static|type|mod)[[:space:]]",
  )
});
static DOC_LINE: Rx = LazyLock::new(|| rx(r"^[[:space:]]*(///|//!)"));
static DOC_RUN_LINE: Rx = LazyLock::new(|| rx(r"^[[:space:]]*//[/!]"));
const MAX_DOC_RUN: usize = 12;

/// `N: line` for public items whose previous non-blank, non-attribute line is
/// no doc, at most 3.
fn undocumented(lines: &[&str]) -> Vec<String> {
  let mut found = Vec::new();
  let mut prev = "";
  for (row, line) in lines.iter().enumerate() {
    if hit(&BLANK, line) || hit(&ATTRIBUTE, line) {
      continue;
    }
    if hit(&PUBLIC_ITEM, line) && !hit(&DOC_LINE, prev) {
      found.push(format!("{}: {line}", row + 1));
    }
    prev = line;
  }
  found.truncate(3);
  found
}

/// Doc runs over the cap, at most 2.
fn verbose(lines: &[&str]) -> Vec<String> {
  let mut found = Vec::new();
  let mut run: Option<(usize, usize)> = None;
  let rows = lines
    .iter()
    .map(|line| hit(&DOC_RUN_LINE, line))
    .chain([false]);
  for (row, is_doc) in rows.enumerate() {
    run = match (run, is_doc) {
      (None, true) => Some((row + 1, 1)),
      (Some((start, len)), true) => Some((start, len + 1)),
      (Some((start, len)), false) => {
        if len > MAX_DOC_RUN {
          found.push(format!(
            "{start}: doc block is {len} lines — trim to the essentials"
          ));
        }
        None
      }
      (None, false) => None,
    };
  }
  found.truncate(2);
  found
}

fn advisory(header: &str, rows: &[String]) -> String {
  if rows.is_empty() {
    String::new()
  } else {
    format!("{header}\n{}", rows.join("\n"))
  }
}

/// 90-docs, the missing-doc and verbose-doc advisories.
pub(crate) fn docs(f: &RsFile<'_>, is_test: bool) -> Vec<String> {
  if !f.path_has("/src/") || is_test {
    return Vec::new();
  }
  let path = f.path();
  vec![
    advisory(
      &format!("Public items missing a /// doc comment in {path} — add a concise one-line doc:"),
      &undocumented(&f.lines),
    ),
    advisory(
      &format!("Verbose doc comment in {path} — docs must be present but concise:"),
      &verbose(&f.lines),
    ),
  ]
}

#[cfg(test)]
#[path = "tests/docs_test.rs"]
mod tests;
