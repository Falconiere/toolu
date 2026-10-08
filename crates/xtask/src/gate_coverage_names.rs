//! Command paths and hook ids for gate-coverage discovery.

use std::path::Path;

use regex::Regex;
use serde_json::Value;

pub(crate) fn commands_of(entry: &Value) -> Vec<String> {
  if let Some(hooks) = entry.get("hooks").and_then(Value::as_array) {
    return hooks
      .iter()
      .filter_map(|hook| hook.get("command").and_then(Value::as_str))
      .filter(|command| !command.is_empty())
      .map(str::to_owned)
      .collect();
  }
  entry
    .get("command")
    .and_then(Value::as_str)
    .filter(|command| !command.is_empty())
    .map(|command| vec![command.to_owned()])
    .unwrap_or_default()
}

pub(crate) fn normalize_command(command: &str) -> String {
  if let Some(bundle) = bundle_path(command) {
    return bundle;
  }
  let trimmed = command.trim();
  let unquoted = unquote(trimmed);
  unquoted
    .replace("${CLAUDE_PLUGIN_ROOT}/", "")
    .replace("${PLUGIN_ROOT}/", "")
}

fn bundle_path(command: &str) -> Option<String> {
  let expr = Regex::new(r"hooks/dist/[a-z0-9-]+\.js").ok()?;
  expr.find(command).map(|found| found.as_str().to_owned())
}

fn unquote(text: &str) -> &str {
  let bytes = text.as_bytes();
  let (Some(&first), Some(&last)) = (bytes.first(), bytes.last()) else {
    return text;
  };
  if bytes.len() >= 2 && ((first == b'"' && last == b'"') || (first == b'\'' && last == b'\'')) {
    return text.get(1..text.len() - 1).unwrap_or(text);
  }
  text
}

pub(crate) fn make_id(
  plugin: &str,
  kind: &str,
  event: &str,
  command: &str,
  matcher: &str,
) -> String {
  let base = sanitize(base_name(command));
  if matcher.is_empty() {
    return format!("{plugin}:{kind}:{event}:{base}");
  }
  let short: String = sanitize(matcher).chars().take(40).collect();
  format!("{plugin}:{kind}:{event}:{base}:{short}")
}

fn sanitize(text: &str) -> String {
  let mut out = String::new();
  let mut spaced = false;
  for ch in text.chars() {
    if ch == '"' || ch == '\'' {
      continue;
    }
    if ch.is_whitespace() {
      if !spaced {
        out.push('_');
        spaced = true;
      }
    } else {
      spaced = false;
      out.push(ch);
    }
  }
  out
}

fn base_name(path: &str) -> &str {
  path.rsplit(['/', '\\']).next().unwrap_or(path)
}

pub(crate) fn native_names(root: &Path) -> Result<Vec<String>, String> {
  let rel = "plugins/toolu/hooks/src/pre-tools/builtins.ts";
  let text =
    std::fs::read_to_string(root.join(rel)).map_err(|err| format!("cannot read {rel}: {err}"))?;
  let start = text
    .find("export const NATIVE_MODULES")
    .ok_or_else(|| format!("{rel}: NATIVE_MODULES missing"))?;
  let rest = text
    .get(start..)
    .ok_or_else(|| format!("{rel}: NATIVE_MODULES missing"))?;
  let end = rest
    .find("} as const")
    .ok_or_else(|| format!("{rel}: NATIVE_MODULES missing"))?;
  let body = rest
    .get(..end)
    .ok_or_else(|| format!("{rel}: NATIVE_MODULES missing"))?;
  let expr = Regex::new(r#""([a-z0-9-]+)":"#).map_err(|err| format!("{rel}: {err}"))?;
  let mut names = expr
    .captures_iter(body)
    .filter_map(|caps| caps.get(1).map(|item| item.as_str().to_owned()))
    .collect::<Vec<_>>();
  names.sort();
  Ok(names)
}

pub(crate) fn dir_names(dir: &Path) -> Result<Vec<String>, String> {
  if !dir.is_dir() {
    return Ok(Vec::new());
  }
  let mut names = Vec::new();
  for entry in
    std::fs::read_dir(dir).map_err(|err| format!("cannot list {}: {err}", dir.display()))?
  {
    let entry = entry.map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
    if entry
      .file_type()
      .map_err(|err| format!("cannot list {}: {err}", dir.display()))?
      .is_dir()
    {
      names.push(entry.file_name().to_string_lossy().into_owned());
    }
  }
  names.sort();
  Ok(names)
}

#[cfg(test)]
#[path = "tests/gate_coverage_names_test.rs"]
mod tests;
