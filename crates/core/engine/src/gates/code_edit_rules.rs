//! The code-edit-rules PreToolUse advisory, with TypeScript's tolerant JSON fields.

use std::path::Path;

use serde_json::Value;
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::config::load::is_file;
use toolu_runtime::config::settings::CODE_EDIT_RULES;
use toolu_runtime::registry::rule::RuleContext;

use super::gate_paths::repo_relative;
use super::pattern::matches;
use super::{file_path, gate_settings_dir};
use crate::gate::Gate;

/// The code-edit-rules built-in gate.
pub(crate) struct CodeEditRules;
/// Its singleton in the ordered pre-tool table.
pub(crate) static CODE_EDIT: CodeEditRules = CodeEditRules;

fn scalar_text(value: &Value) -> Option<String> {
  match value {
    Value::String(text) => Some(text.clone()),
    Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
    Value::Null | Value::Array(_) | Value::Object(_) => None,
  }
}

fn joined(value: Option<&Value>) -> String {
  let Some(Value::Array(items)) = value else {
    return String::new();
  };
  let parts: Option<Vec<_>> = items
    .iter()
    .map(|item| {
      if item.is_null() {
        Some(String::new())
      } else {
        scalar_text(item)
      }
    })
    .collect();
  parts.map_or_else(String::new, |parts| parts.join(" + "))
}

fn patterns(value: Option<&Value>) -> Vec<String> {
  let Some(Value::Array(items)) = value else {
    return Vec::new();
  };
  items
    .iter()
    .filter_map(|item| {
      if item.is_null() {
        Some("null".to_owned())
      } else {
        scalar_text(item)
      }
    })
    .collect()
}

fn read_rules(path: &Path) -> Vec<Value> {
  let parsed = std::fs::read_to_string(path)
    .ok()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
  parsed
    .and_then(|value| value.as_object()?.get("rules")?.as_array().cloned())
    .unwrap_or_default()
}

fn docs_for(rules: &[Value], rel: &str) -> String {
  for rule in rules {
    let Some(rule) = rule.as_object() else {
      continue;
    };
    let pattern = rule
      .get("match")
      .filter(|value| **value != Value::Bool(false))
      .and_then(scalar_text);
    let Some(pattern) = pattern.filter(|pattern| !pattern.is_empty()) else {
      continue;
    };
    if !matches(&pattern, rel) {
      continue;
    }
    let mut docs = vec![joined(rule.get("docs"))];
    if patterns(rule.get("when_path_matches"))
      .iter()
      .any(|cond| matches(cond, rel))
    {
      docs.push(joined(rule.get("extra_docs")));
    }
    return docs
      .into_iter()
      .filter(|doc| !doc.is_empty() && doc != "null")
      .collect::<Vec<_>>()
      .join(" + ");
  }
  String::new()
}

impl Gate for CodeEditRules {
  fn name(&self) -> &'static str {
    "code-edit-rules"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    let NormalizedEvent::ToolPre { tool, .. } = event else {
      return Ok(Decision::Allow);
    };
    if !matches!(tool.name.as_str(), "Edit" | "Write" | "MultiEdit") {
      return Ok(Decision::Allow);
    }
    let Some(dir) = gate_settings_dir(ctx) else {
      return Ok(Decision::Allow);
    };
    let file = dir.join(CODE_EDIT_RULES);
    if !is_file(&file) {
      return Ok(Decision::Allow);
    }
    let path = file_path(event);
    if path.is_empty() {
      return Ok(Decision::Allow);
    }
    let rel = repo_relative(path, ctx.project_root);
    let docs = docs_for(&read_rules(&file), &rel);
    if docs.is_empty() {
      return Ok(Decision::Allow);
    }
    let message = Text::new(format!("File: {rel}\nApply these rules: {docs}"))
      .map_err(|err| err.to_string())?;
    Ok(Decision::Advisory { message })
  }
}

#[cfg(test)]
#[path = "tests/code_edit_rules_test.rs"]
mod tests;
