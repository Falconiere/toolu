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

/// What a native `hooks.json` command runs: `hook <name>`.
pub(crate) fn hook_module(command: &str) -> Option<String> {
  toolu_protocol::launcher::hook_name(command).map(|name| format!("hook {name}"))
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

/// The engine's built-in gate table.
const BUILTINS: &str = "crates/core/engine/src/builtins.rs";

/// The gate modules of `table` (`PRE_TOOL` or `POST_TOOL`) in `builtins.rs`, in
/// table order, as `(name, source)`: `bash-commands`, `crates/core/engine/src/gates/bash_commands.rs`.
pub(crate) fn builtin_gates(root: &Path, table: &str) -> Result<Vec<(String, String)>, String> {
  let text = std::fs::read_to_string(root.join(BUILTINS))
    .map_err(|err| format!("cannot read {BUILTINS}: {err}"))?;
  let uses = Regex::new(r"use crate::gates::([a-z_]+)::([A-Z_]+);")
    .map_err(|err| format!("{BUILTINS}: {err}"))?;
  let modules: Vec<(String, String)> = uses
    .captures_iter(&text)
    .filter_map(|caps| {
      Some((
        caps.get(2)?.as_str().to_owned(),
        caps.get(1)?.as_str().to_owned(),
      ))
    })
    .collect();
  let start = text
    .find(&format!("pub const {table}"))
    .ok_or_else(|| format!("{BUILTINS}: {table} missing"))?;
  let body = text
    .get(start..)
    .and_then(|rest| rest.split_once("];"))
    .map(|(body, _)| body)
    .ok_or_else(|| format!("{BUILTINS}: {table} missing"))?;
  let consts = Regex::new(r"&([A-Z_]+)\b").map_err(|err| format!("{BUILTINS}: {err}"))?;
  consts
    .captures_iter(body)
    .filter_map(|caps| caps.get(1))
    .map(|name| {
      let module = modules
        .iter()
        .find(|(constant, _)| constant == name.as_str())
        .map(|(_, module)| module.clone())
        .ok_or_else(|| format!("{BUILTINS}: no gate module for {}", name.as_str()))?;
      let source = format!("crates/core/engine/src/gates/{module}.rs");
      Ok((module.replace('_', "-"), source))
    })
    .collect()
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
