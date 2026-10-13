//! Bounded log summary: distinct error lines, recent tail, and total count.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use toolu_runtime::env::Env;

use super::cap;

static ISO_TIME: LazyLock<Option<Regex>> =
  LazyLock::new(|| Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9:.]+([Zz]|[+-][0-9:]+)?").ok());
static BRACKET_TIME: LazyLock<Option<Regex>> =
  LazyLock::new(|| Regex::new(r"^\[[0-9]{2}:[0-9]{2}:[0-9]{2}([.,][0-9]+)?\]").ok());

fn errorish(line: &str) -> bool {
  let lower = line.to_ascii_lowercase();
  [
    "error",
    "warn",
    "panic",
    "fail",
    "exception",
    "fatal",
    "traceback",
  ]
  .iter()
  .any(|word| lower.contains(word))
}

fn key(line: &str) -> String {
  let iso = ISO_TIME.as_ref().map_or_else(
    || line.to_owned(),
    |re| re.replace(line, "<TS>").into_owned(),
  );
  BRACKET_TIME
    .as_ref()
    .map_or_else(|| iso.clone(), |re| re.replace(&iso, "<TS>").into_owned())
}

fn selected(lines: &[&str], max_lines: usize) -> (Vec<String>, Vec<String>, bool) {
  let kept: Vec<String> = lines
    .iter()
    .map(|line| line.trim_end_matches('\r').to_owned())
    .collect();
  let mut seen = HashSet::new();
  let errors: Vec<String> = kept
    .iter()
    .filter(|line| errorish(line) && seen.insert(key(line)))
    .cloned()
    .collect();
  let error_cap = (max_lines.saturating_mul(3) / 5).max(1);
  let error_count = errors.len().min(error_cap);
  let used =
    error_count + usize::from(!errors.is_empty()) + usize::from(errors.len() > error_count) + 1;
  let tail_cap = max_lines.saturating_sub(used + 1);
  let candidates = kept
    .get(kept.len().saturating_sub(max_lines)..)
    .unwrap_or(&[]);
  let tail_count = candidates.len().min(tail_cap);
  let tail = candidates
    .iter()
    .rev()
    .take(tail_count)
    .rev()
    .cloned()
    .collect();
  let truncated =
    errors.len() > error_count || tail_count < candidates.len() || kept.len() > max_lines;
  (
    errors.iter().take(error_count).cloned().collect(),
    tail,
    truncated,
  )
}

fn bounded(lines: &[String], max_bytes: usize) -> String {
  let mut out = String::new();
  for line in lines {
    if out.len() + line.len() + 1 > max_bytes {
      break;
    }
    out.push_str(line);
    out.push('\n');
  }
  if out.lines().count() < lines.len() {
    let marker = format!("... (output truncated to stay under {max_bytes} bytes)");
    if out.len() + marker.len() < max_bytes {
      out.push_str(&marker);
    }
  }
  out.trim_end_matches('\n').to_owned()
}

/// Summarize a log in bounded text or named JSON fields.
pub(super) fn summarize(lines: &[&str], json: bool, env: &Env) -> String {
  let max_lines = cap(env, "DEBUG_MAX_LINES", 100);
  let max_bytes = cap(env, "DEBUG_MAX_BYTES", 65_536);
  let (errors, tail, truncated) = selected(lines, max_lines);
  if json {
    return serde_json::json!({
      "errors": errors, "tail": tail, "total_lines": lines.len(), "truncated": truncated
    })
    .to_string();
  }
  let all_errors = lines
    .iter()
    .map(|line| line.trim_end_matches('\r'))
    .filter(|line| errorish(line))
    .map(key)
    .collect::<HashSet<_>>()
    .len();
  let over = all_errors.saturating_sub(errors.len());
  let mut out = vec![format!(
    "ERRORS/WARNINGS ({}{}):",
    if all_errors == 0 {
      String::new()
    } else {
      errors.len().to_string()
    },
    if over > 0 { "+" } else { "" }
  )];
  out.extend(errors.iter().map(|line| format!("  {line}")));
  if over > 0 {
    out.push(format!("  ... (+{over} more)"));
  }
  out.push(format!(
    "TAIL (last {} of {} lines):",
    tail.len(),
    lines.len()
  ));
  out.extend(tail.iter().map(|line| format!("  {line}")));
  out.push(format!("TOTAL lines: {}", lines.len()));
  bounded(&out, max_bytes)
}

#[cfg(test)]
#[path = "tests/log_test.rs"]
mod tests;
