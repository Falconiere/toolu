//! An `OpenCode` hook call as the Claude-shaped payload every toolu hook reads.
//! The shim sends `{"directory": <ctx.location.directory>, "event": <the hook's
//! event>}`; the events are those of `@opencode/plugin` 2.x
//! (<https://opencode.ai/v2/docs/build/plugins/server/>), probed on `OpenCode` 2.2:
//!
//! - `tool.execute.before`: `{tool, sessionID, agent, messageID, id, input}`;
//! - `tool.execute.after`: the same, with `status` and `result` (`completed`) or
//!   `error` (`error`);
//! - `session.prompt`: `{sessionID, messageID, prompt: {text, files}, delivery}`;
//! - `session.compaction` and `session.start`: `{sessionID}`, the latter with an
//!   optional `source`.

use std::path::Path;

use serde_json::{Map, Value};

use super::Hook;
use super::tools::{claude_input, claude_name};

/// What the shim sends the bridge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Call {
  /// The plugin instance's location directory.
  pub directory: Option<String>,
  /// The `OpenCode` event, as the hook received it.
  pub event: Map<String, Value>,
}

impl Call {
  /// Parse the shim's JSON: an object whose `event` is an object.
  ///
  /// # Errors
  /// When `text` is not that object.
  pub fn parse(text: &str) -> Result<Call, String> {
    let doc: Value =
      serde_json::from_str(text).map_err(|err| format!("the OpenCode call is not JSON: {err}"))?;
    let Value::Object(mut doc) = doc else {
      return Err("the OpenCode call is not a JSON object".to_owned());
    };
    let Some(Value::Object(event)) = doc.remove("event") else {
      return Err("the OpenCode call has no event object".to_owned());
    };
    let directory = doc
      .get("directory")
      .and_then(Value::as_str)
      .filter(|dir| !dir.is_empty())
      .map(str::to_owned);
    Ok(Call { directory, event })
  }

  fn text(&self, key: &str) -> Option<&str> {
    self
      .event
      .get(key)
      .and_then(Value::as_str)
      .filter(|text| !text.is_empty())
  }

  /// The working directory a hook runs in: a shell call's `workdir`, resolved
  /// against the location, else the location.
  pub fn cwd(&self) -> Option<String> {
    let workdir = self
      .event
      .get("input")
      .and_then(|input| input.get("workdir"))
      .and_then(Value::as_str)
      .filter(|dir| !dir.is_empty());
    match (workdir, self.directory.as_deref()) {
      (Some(dir), Some(base)) => Some(Path::new(base).join(dir).display().to_string()),
      (Some(dir), None) => Some(dir.to_owned()),
      (None, base) => base.map(str::to_owned),
    }
  }

  /// The Claude-shaped payload of `hook` for this call.
  pub fn payload(&self, hook: Hook) -> Map<String, Value> {
    let mut out = Map::new();
    let mut put = |key: &str, value: Value| {
      out.insert(key.to_owned(), value);
    };
    put("hook_event_name", hook.claude_event().into());
    put(
      "session_id",
      self.text("sessionID").unwrap_or_default().into(),
    );
    if let Some(cwd) = self.cwd() {
      put("cwd", cwd.into());
    }
    match hook {
      Hook::SessionStart => put("source", self.text("source").unwrap_or("startup").into()),
      Hook::Prompt => put("prompt", self.prompt().into()),
      Hook::Compaction => put("trigger", "auto".into()),
      Hook::ToolBefore => self.tool(&mut put),
      Hook::ToolAfter => {
        self.tool(&mut put);
        put("tool_response", self.response());
      }
    }
    out
  }

  fn prompt(&self) -> String {
    self
      .event
      .get("prompt")
      .and_then(|prompt| prompt.get("text"))
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_owned()
  }

  fn tool(&self, put: &mut impl FnMut(&str, Value)) {
    let name = self.text("tool").unwrap_or_default();
    let input = self
      .event
      .get("input")
      .and_then(Value::as_object)
      .cloned()
      .unwrap_or_default();
    put("tool_name", claude_name(name).into());
    put("tool_input", Value::Object(claude_input(name, &input)));
    put("tool_use_id", self.text("id").unwrap_or_default().into());
  }

  /// A completed call's `result.output` object with its `metadata` and its text
  /// `content` joined, plus `exit_code` when the output or metadata reports one;
  /// a failed call's `{"error": message}`.
  fn response(&self) -> Value {
    if self.text("status") == Some("error") {
      let message = self
        .event
        .get("error")
        .and_then(|error| error.get("message").or(Some(error)))
        .map(|error| {
          error
            .as_str()
            .map_or_else(|| error.to_string(), str::to_owned)
        })
        .unwrap_or_default();
      return Value::Object(Map::from_iter([("error".to_owned(), message.into())]));
    }
    let result = self.event.get("result");
    let field = |key: &str| result.and_then(|result| result.get(key));
    let mut out = match field("output") {
      Some(Value::Object(output)) => output.clone(),
      Some(Value::Null) | None => Map::new(),
      Some(other) => Map::from_iter([("output".to_owned(), other.clone())]),
    };
    if let Some(metadata) = field("metadata") {
      out.insert("metadata".to_owned(), metadata.clone());
    }
    if let Some(text) = field("content").and_then(content_text) {
      out.insert("content".to_owned(), text.into());
    }
    if let Some(code) = exit_code(&out) {
      out.entry("exit_code").or_insert(code.into());
    }
    Value::Object(out)
  }
}

/// The text parts of a `content` array, joined by newlines.
fn content_text(content: &Value) -> Option<String> {
  let parts: Vec<&str> = content
    .as_array()?
    .iter()
    .filter_map(|part| part.get("text").and_then(Value::as_str))
    .collect();
  (!parts.is_empty()).then(|| parts.join("\n"))
}

/// The first integer exit status among `exitCode`, `exit_code` and `exit`, in
/// the output and then its metadata.
fn exit_code(output: &Map<String, Value>) -> Option<i64> {
  let metadata = output.get("metadata").and_then(Value::as_object);
  [Some(output), metadata]
    .into_iter()
    .flatten()
    .find_map(|map| {
      ["exitCode", "exit_code", "exit"]
        .into_iter()
        .find_map(|key| map.get(key).and_then(Value::as_i64))
    })
}

/// The value a `hooks.json` matcher of `hook` is tested against: the tool name,
/// the session source or the compaction trigger; `None` when the event has no
/// matcher, as `UserPromptSubmit` has none.
pub fn matcher_subject(hook: Hook, payload: &Map<String, Value>) -> Option<&str> {
  let key = match hook {
    Hook::SessionStart => "source",
    Hook::Compaction => "trigger",
    Hook::ToolBefore | Hook::ToolAfter => "tool_name",
    Hook::Prompt => return None,
  };
  payload.get(key).and_then(Value::as_str)
}

#[cfg(test)]
#[path = "tests/event_test.rs"]
mod tests;
