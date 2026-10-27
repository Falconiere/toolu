//! The only place `toolu` writes to its standard streams.

use std::io::{IsTerminal as _, Write};

use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::term::{Style, style_of};

/// What standard output can show: rails at a terminal, else the plain text.
pub(crate) fn style() -> Style {
  style_of(&Env::process(), std::io::stdout().is_terminal())
}

// A closed stream has nowhere left to report to, so a failed write is dropped.

/// Print the outcome's lines on standard output and standard error.
pub(crate) fn emit(outcome: &Outcome) {
  emit_with(outcome, true);
}

/// Print a statusline result without adding a newline to its stdout.
pub(crate) fn emit_raw(outcome: &Outcome) {
  emit_with(outcome, false);
}

fn emit_with(outcome: &Outcome, newline: bool) {
  if let Some(line) = &outcome.stdout {
    if newline {
      writeln!(std::io::stdout().lock(), "{line}").ok();
    } else {
      write!(std::io::stdout().lock(), "{line}").ok();
    }
  }
  if let Some(line) = &outcome.stderr {
    writeln!(std::io::stderr().lock(), "{line}").ok();
  }
}

#[cfg(test)]
#[path = "tests/output_test.rs"]
mod tests;
