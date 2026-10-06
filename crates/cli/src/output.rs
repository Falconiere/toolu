//! The only place `toolu` writes to its standard streams.

use std::io::Write;

use toolu_runtime::cli::Outcome;

// A closed stream has nowhere left to report to, so a failed write is dropped.

/// Print the outcome's lines on standard output and standard error.
pub(crate) fn emit(outcome: &Outcome) {
  if let Some(line) = &outcome.stdout {
    writeln!(std::io::stdout().lock(), "{line}").ok();
  }
  if let Some(line) = &outcome.stderr {
    writeln!(std::io::stderr().lock(), "{line}").ok();
  }
}

#[cfg(test)]
#[path = "tests/output_test.rs"]
mod tests;
