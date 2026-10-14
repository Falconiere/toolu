//! The check vocabulary of the statusline cases, with diffs that show the whole output.

use regex::Regex;
use serde_json::Value;

use crate::golden_statusline_sandbox::Sandbox;
use crate::golden_statusline_value::TestResult;
use crate::golden_statusline_value::{bad, field, str_of};

/// Strip ANSI colour codes.
pub(crate) fn plain(text: &str) -> TestResult<String> {
  Ok(
    Regex::new("\u{1b}\\[[0-9;]*m")?
      .replace_all(text, "")
      .into_owned(),
  )
}

fn holds(op: &str, out: &str, value: &str) -> TestResult<bool> {
  Ok(match op {
    "contains" => out.contains(value),
    "absent" => !out.contains(value),
    "equal" => out == value,
    "ends" => out.ends_with(value),
    "not-match" => !Regex::new(value)?.is_match(out),
    other => return Err(bad(format!("unknown check op: {other}"))),
  })
}

/// Every check must hold over `out`.
pub(crate) fn run_checks(sb: &Sandbox, out: &str, checks: &[Value]) -> TestResult<()> {
  for check in checks {
    let op = str_of(check, "op")?;
    let value = sb.string(field(check, "value")?)?;
    if !holds(op, out, &value)? {
      return Err(bad(format!(
        "check {op} {value:?} failed against output {out:?}"
      )));
    }
  }
  Ok(())
}
