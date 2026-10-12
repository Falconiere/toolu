//! Stdout of a `toolu epic` run as one JSON document.

use std::process::Output;

use serde_json::Value;

/// Stdout as one JSON document; the error names the text that was not.
pub(crate) fn doc(output: &Output) -> Result<Value, String> {
  serde_json::from_slice(&output.stdout)
    .map_err(|e| format!("{e}: {}", String::from_utf8_lossy(&output.stdout)))
}
