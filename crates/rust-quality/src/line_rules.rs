//! The line rules: lint suppression (30) and `unsafe` code (40). The unsafe
//! scan drops `//` and `/* */` comments first, as the awk did; string literals
//! are not tracked.

use std::sync::LazyLock;

use crate::file::{RsFile, Rx, hit, rx};

static SUPPRESSION: Rx = LazyLock::new(|| {
  rx(r"^[[:space:]]*#!?\[(allow|expect)\(|^[[:space:]]*#!?\[cfg_attr\([^\]]*\b(allow|expect)\b")
});
/// In a test file the inner `#![...]` header is accepted.
static OUTER_SUPPRESSION: Rx = LazyLock::new(|| {
  rx(r"^[[:space:]]*#\[(allow|expect)\(|^[[:space:]]*#\[cfg_attr\([^\]]*\b(allow|expect)\b")
});

/// 30-suppression.
pub(crate) fn suppression(f: &RsFile<'_>) -> Option<String> {
  let in_tests = f.path_has("/tests/");
  let pattern = if in_tests {
    &OUTER_SUPPRESSION
  } else {
    &SUPPRESSION
  };
  if !f.lines.iter().any(|line| hit(pattern, line)) {
    return None;
  }
  let hint = if in_tests {
    concat!(
      " In a test file only the file-level #![",
      "allow(...)] header is accepted."
    )
  } else {
    ""
  };
  Some(format!(
    concat!(
      "Forbidden lint suppression (#[",
      "allow]/#[",
      "expect]/cfg_attr allow) in {} — remove it and fix the underlying warning in code. For unsafe_code, override in Cargo.toml [lints.rust].{}"
    ),
    f.path(),
    hint
  ))
}

static UNSAFE: Rx = LazyLock::new(|| rx(r"(^|[^A-Za-z0-9_])unsafe[ \t]*(\{|fn )"));

/// `line` without its block comments; the flag says a block is still open.
fn without_blocks(mut line: String) -> (String, bool) {
  while let Some((before, opened)) = line.split_once("/*") {
    let Some((_, after)) = opened.split_once("*/") else {
      return (before.to_owned(), true);
    };
    line = format!("{before}{after}");
  }
  (line, false)
}

/// Whether any line holds `unsafe {` or `unsafe fn` outside a comment.
fn has_unsafe(lines: &[&str]) -> bool {
  let mut in_block = false;
  for raw in lines {
    let rest = if in_block {
      match raw.split_once("*/") {
        Some((_, rest)) => rest,
        None => continue,
      }
    } else {
      raw
    };
    let (code, open) = without_blocks(rest.to_owned());
    in_block = open;
    let code = code
      .split_once("//")
      .map_or(code.as_str(), |(before, _)| before);
    if hit(&UNSAFE, code) {
      return true;
    }
  }
  false
}

/// 40-unsafe, unless the path names an exempt crate.
pub(crate) fn unsafe_code(f: &RsFile<'_>) -> Option<String> {
  let exempt = f
    .limits
    .unsafe_exemptions
    .iter()
    .any(|name| !name.is_empty() && f.path_has(name));
  (!exempt && has_unsafe(&f.lines)).then(|| {
    format!(
      "Forbidden unsafe code in {} — refactor to safe alternative. Add crate to settings/rust-unsafe-exemptions.txt if it legitimately needs unsafe (FFI, sandboxing).",
      f.path()
    )
  })
}

#[cfg(test)]
#[path = "tests/line_rules_test.rs"]
mod tests;
