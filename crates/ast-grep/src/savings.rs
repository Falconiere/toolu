//! Native post-tool byte accounting.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};

use crate::launched::{program_at, program_indexes};

fn jq_or<'a>(value: Option<&'a Value>, fallback: &'a Value) -> &'a Value {
  match value {
    Some(Value::Null | Value::Bool(false)) | None => fallback,
    Some(value) => value,
  }
}

fn jq_raw(value: &Value) -> String {
  match value {
    Value::String(text) => text.trim_end_matches('\n').to_owned(),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
      serde_json::to_string_pretty(value).unwrap_or_default()
    }
  }
}

fn response_value(response: Option<&Value>) -> String {
  let null = Value::Null;
  let response = response.unwrap_or(&null);
  if let Value::Object(map) = response {
    let compact = Value::String(response.to_string());
    let selected = map
      .get("content")
      .filter(|value| !value.is_null() && *value != &Value::Bool(false))
      .or_else(|| {
        map
          .get("stdout")
          .filter(|value| !value.is_null() && *value != &Value::Bool(false))
      })
      .or_else(|| {
        map
          .get("output")
          .filter(|value| !value.is_null() && *value != &Value::Bool(false))
      })
      .unwrap_or(&compact);
    return jq_raw(selected);
  }
  match response {
    Value::String(_) => jq_raw(response),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
      response.to_string()
    }
  }
}

fn returned_bytes(response: Option<&Value>) -> Option<usize> {
  let text = response_value(response);
  (!text.is_empty()).then_some(text.len())
}

fn runs_ast_grep(command: &str) -> bool {
  if !command.contains("sg") && !command.contains("ast-grep") {
    return false;
  }
  toolu_shell::analyze(command)
    .commands
    .iter()
    .any(|command| {
      program_indexes(command)
        .into_iter()
        .any(|at| matches!(program_at(command, at), Some("ast-grep" | "sg")))
    })
}

fn kind(tool: &str, input: &serde_json::Map<String, Value>) -> Option<&'static str> {
  match tool {
    "Read" => Some("read"),
    "Grep" => Some("grep"),
    "Glob" => Some("glob"),
    "Bash" | "Shell" => input
      .get("command")
      .map(jq_raw)
      .filter(|command| runs_ast_grep(command))
      .map(|_| "ast-grep"),
    _ => None,
  }
}

fn full_bytes(path: Option<&Value>, cwd: &Path) -> u64 {
  let Some(path) = path.map(jq_raw).filter(|path| !path.is_empty()) else {
    return 0;
  };
  let file = cwd.join(path);
  fs::metadata(file)
    .ok()
    .filter(fs::Metadata::is_file)
    .map_or(0, |meta| meta.len())
}

/// Sanitize a session id for the ledger filename.
pub(crate) fn session_id(raw: Option<&Value>) -> String {
  let unknown = Value::String("unknown".to_owned());
  let id: String = jq_raw(jq_or(raw, &unknown))
    .chars()
    .filter(|char| char.is_ascii_alphanumeric() || *char == '-')
    .collect();
  if id.is_empty() {
    "unknown".to_owned()
  } else {
    id
  }
}

/// Config-root directory that holds `<session_id>.jsonl` ledgers.
pub(crate) fn byte_savings_dir(env: &toolu_runtime::env::Env) -> PathBuf {
  let root = env
    .get("TOOLU_CONFIG_DIR")
    .or_else(|| env.get("CODEX_HOME"))
    .or_else(|| env.get("CLAUDE_CONFIG_DIR"))
    .map_or_else(|| env.home().join(".claude"), PathBuf::from);
  root.join("toolu").join("byte-savings")
}

fn ledger_path(ctx: &RuleContext<'_>) -> PathBuf {
  byte_savings_dir(ctx.env).join(format!("{}.jsonl", session_id(ctx.raw.get("session_id"))))
}

#[derive(serde::Serialize)]
struct LedgerLine<'a> {
  kind: &'a str,
  returned: usize,
  full: u64,
}

fn append(path: &Path, kind: &str, returned: usize, full: u64) -> bool {
  let Some(parent) = path.parent() else {
    return false;
  };
  if fs::create_dir_all(parent).is_err() {
    return false;
  }
  let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
    return false;
  };
  let Ok(mut line) = serde_json::to_vec(&LedgerLine {
    kind,
    returned,
    full,
  }) else {
    return false;
  };
  line.push(b'\n');
  file.write_all(&line).is_ok()
}

/// The compiled post-tool rule.
struct ByteSavings;

/// `byte-savings` manifest's compiled rule.
pub const BYTE_SAVINGS: &dyn Rule = &ByteSavings;

impl Rule for ByteSavings {
  fn spec(&self) -> &'static str {
    "ast-grep@toolu"
  }
  fn name(&self) -> &'static str {
    "byte-savings"
  }
  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPost
  }

  fn applies(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> bool {
    event.tool().is_some_and(|tool| {
      matches!(
        tool.name.as_str(),
        "Read" | "Grep" | "Glob" | "Bash" | "Shell"
      )
    })
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    let Some(tool) = event.tool() else {
      return Decision::Allow;
    };
    let Some(kind) = kind(tool.name.as_str(), &tool.input) else {
      return Decision::Allow;
    };
    let Some(returned) = returned_bytes(ctx.raw.get("tool_response")) else {
      return Decision::Allow;
    };
    let full = if kind == "read" {
      full_bytes(
        tool.input.get("file_path"),
        ctx.cwd.unwrap_or(ctx.project_root),
      )
    } else {
      0
    };
    let path = ledger_path(ctx);
    // Best effort: a ledger that cannot be written never blocks the tool.
    append(&path, kind, returned, full);
    Decision::Allow
  }
}

#[cfg(test)]
#[path = "tests/savings_test.rs"]
mod tests;
