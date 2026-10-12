//! Stdout of a `toolu epic` run.

use std::process::Output;

/// Stdout as text.
pub(crate) fn out(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}
