//! Read `ast-grep scan --json` output into item matches.

use serde_json::Value;

/// One matched item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Match {
  /// The file as ast-grep printed it.
  pub(super) file: String,
  /// First line, zero-based.
  pub(super) start: usize,
  /// Last line, zero-based.
  pub(super) end: usize,
  /// The captured `$NAME`.
  pub(super) name: String,
}

fn line(value: &Value, edge: &str) -> Option<usize> {
  usize::try_from(value.get("range")?.get(edge)?.get("line")?.as_u64()?).ok()
}

fn one(value: &Value) -> Option<Match> {
  Some(Match {
    file: value.get("file")?.as_str()?.to_owned(),
    start: line(value, "start")?,
    end: line(value, "end")?,
    name: value
      .get("metaVariables")?
      .get("single")?
      .get("NAME")?
      .get("text")?
      .as_str()?
      .to_owned(),
  })
}

/// The matches in `stdout`; blank output has none.
///
/// # Errors
/// Output that is not a JSON array of matches with a file, range and `$NAME`.
pub(super) fn matches(stdout: &str) -> Result<Vec<Match>, String> {
  if stdout.trim().is_empty() {
    return Ok(Vec::new());
  }
  let Ok(Value::Array(values)) = serde_json::from_str::<Value>(stdout) else {
    return Err("ast-grep printed invalid JSON".to_owned());
  };
  values
    .iter()
    .map(|value| {
      one(value).ok_or_else(|| "ast-grep printed a match without a file, range or name".to_owned())
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/parse_test.rs"]
mod tests;
