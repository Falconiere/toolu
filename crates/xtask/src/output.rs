//! The only place xtask writes to its standard streams.

use std::io::Write;

// A closed stream has nowhere left to report to, so a failed write is dropped
// with `.ok()` rather than turned into a second failure.

/// Print one line on standard output, flushed so it interleaves with child output.
pub(crate) fn say(line: &str) {
  let mut out = std::io::stdout().lock();
  writeln!(out, "{line}").and_then(|()| out.flush()).ok();
}

/// Print one line on standard error.
pub(crate) fn error(line: &str) {
  writeln!(std::io::stderr().lock(), "{line}").ok();
}

/// Print each finding on standard error and return the verdict they make.
pub(crate) fn findings<T: std::fmt::Display>(task: &str, found: &[T]) -> crate::Verdict {
  for finding in found {
    error(&finding.to_string());
  }
  if found.is_empty() {
    say(&format!("{task}: ok"));
    crate::Verdict::Clean
  } else {
    error(&format!("{task}: {} finding(s)", found.len()));
    crate::Verdict::Findings
  }
}

#[cfg(test)]
#[path = "tests/output_test.rs"]
mod tests;
