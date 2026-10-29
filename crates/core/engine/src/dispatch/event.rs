//! The event and context one walk hands its modules (`toolEvent` and
//! `toolContext` in `dispatch-context.ts`): a `NormalizedEvent` and
//! `RuleContext` for gates and rules.

use serde_json::{Map, Value, json};
use std::cell::OnceCell;
use toolu_protocol::event::HostEvent;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::registry::rule::{EditOperation, EditSplit, RuleContext};
use toolu_state::edit_records::EditRecord;

use super::Phase;
use super::output::plain;
use super::session::Session;

/// One path of a split patch: what `TOOLU_EDIT_*` and `ctx.edit` carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditFields {
  pub(crate) operation: toolu_state::edit_records::EditOperation,
  /// `record.from ?? ""`.
  pub(crate) from: String,
  /// `record.moved_to ?? ""`.
  pub(crate) moved_to: String,
}

impl EditFields {
  /// The fields of `record`.
  pub(crate) fn of(record: &EditRecord) -> EditFields {
    EditFields {
      operation: record.operation,
      from: record.from.clone().unwrap_or_default(),
      moved_to: record.moved_to.clone().unwrap_or_default(),
    }
  }
}

/// The text every module of one walk reads, and what it was built from.
pub(crate) struct Payload<'a> {
  pub(crate) text: &'a str,
  /// `text` as `JSON.parse` reads it, keys in JavaScript order; `None` when it is not JSON.
  pub(crate) doc: Option<&'a Ordered>,
  /// A split path renders its full JSON only if an executable needs it.
  pub(crate) synthetic: Option<(&'a Ordered, &'a EditRecord)>,
  pub(crate) rendered: OnceCell<String>,
  /// The walk's tool name: `"Edit"` for a split path, `""` when stdin is not JSON.
  pub(crate) tool_name: &'a str,
  pub(crate) edit: Option<EditFields>,
}

impl Payload<'_> {
  /// Original text, or the per-path JSON when a module first needs it.
  pub(crate) fn text(&self) -> &str {
    self.synthetic.map_or(self.text, |(doc, record)| {
      self.rendered.get_or_init(|| {
        toolu_runtime::json::jq_text(&super::edits::synthetic_edit(doc, record), false)
      })
    })
  }
}

/// What one walk's modules see.
pub(crate) struct View {
  pub(crate) event: NormalizedEvent,
  pub(crate) raw: Map<String, Value>,
}

/// A non-empty string field, else `fallback`.
fn text<'v>(value: Option<&'v Value>, fallback: &'v str) -> &'v str {
  value
    .and_then(Value::as_str)
    .filter(|text| !text.is_empty())
    .unwrap_or(fallback)
}

/// The host event a walk's decisions are encoded for.
pub(crate) fn host_event(event: &NormalizedEvent) -> HostEvent {
  match event {
    NormalizedEvent::ShellPre { .. } => HostEvent::ShellPre,
    NormalizedEvent::ToolPost { .. } => HostEvent::ToolPost,
    NormalizedEvent::ToolPre { .. }
    | NormalizedEvent::SessionStart(_)
    | NormalizedEvent::SessionResume(_)
    | NormalizedEvent::SessionClear(_)
    | NormalizedEvent::SessionUnload(_)
    | NormalizedEvent::Prompt { .. }
    | NormalizedEvent::PreCompact(_)
    | NormalizedEvent::Compaction(_)
    | NormalizedEvent::PermissionEvaluate { .. } => HostEvent::ToolPre,
  }
}

impl View {
  /// Change the split path while keeping the outer patch and event setup.
  pub(crate) fn set_record(&mut self, record: &EditRecord) {
    let update = |input: &mut Map<String, Value>| {
      for (key, value) in [
        ("file_path", record.path.as_str()),
        ("path", record.path.as_str()),
        ("toolu_edit_operation", record.operation.name()),
        (
          "toolu_edit_from",
          record.from.as_deref().unwrap_or_default(),
        ),
        (
          "toolu_edit_moved_to",
          record.moved_to.as_deref().unwrap_or_default(),
        ),
      ] {
        input.insert(key.to_owned(), value.into());
      }
    };
    match &mut self.event {
      NormalizedEvent::ToolPre { tool, .. } | NormalizedEvent::ToolPost { tool, .. } => {
        update(&mut tool.input);
      }
      _ => {}
    }
    if let Some(input) = self
      .raw
      .get_mut("tool_input")
      .and_then(Value::as_object_mut)
    {
      update(input);
    }
  }

  /// The event and context for `payload`.
  ///
  /// # Errors
  /// When the event does not form, which the fallbacks rule out.
  pub(crate) fn of(payload: &Payload<'_>, session: &Session<'_>) -> Result<View, String> {
    let raw = match payload.doc.map(plain) {
      Some(Value::Object(map)) => map,
      Some(_) | None => Map::new(),
    };
    let fields = fields(payload, session, &raw);
    let wire = Value::Object(fields.into_iter().collect());
    let event: NormalizedEvent = serde_json::from_value(wire).map_err(|err| err.to_string())?;
    Ok(View { event, raw })
  }

  /// The context a gate or rule sees for this view.
  pub(crate) fn rule_context<'v>(
    &'v self,
    payload: &'v Payload<'_>,
    session: &'v Session<'_>,
  ) -> RuleContext<'v> {
    RuleContext {
      host: session.host,
      env: &session.env,
      config_root: &session.config_root,
      project_root: &session.project_root,
      cwd: Some(session.cwd()),
      plugin_root: session
        .options
        .lib_dir
        .parent()
        .and_then(std::path::Path::parent),
      raw: &self.raw,
      edit: payload.edit.as_ref().map(|edit| EditSplit {
        operation: runtime_operation(edit.operation),
        from: &edit.from,
        moved_to: &edit.moved_to,
      }),
    }
  }
}

fn runtime_operation(operation: toolu_state::edit_records::EditOperation) -> EditOperation {
  use toolu_state::edit_records::EditOperation as State;
  match operation {
    State::Add => EditOperation::Add,
    State::Update => EditOperation::Update,
    State::Delete => EditOperation::Delete,
    State::Write => EditOperation::Write,
    State::Move => EditOperation::Move,
  }
}

/// The event's fields, in `toolEvent`'s key order.
fn fields(
  payload: &Payload<'_>,
  session: &Session<'_>,
  raw: &Map<String, Value>,
) -> Vec<(String, Value)> {
  let project = session.project_root.to_string_lossy().into_owned();
  let input = raw
    .get("tool_input")
    .filter(|input| input.is_object())
    .cloned()
    .unwrap_or_else(|| json!({}));
  let tool = if payload.tool_name.is_empty() {
    "unknown"
  } else {
    payload.tool_name
  };
  let mut wire = Vec::new();
  let mut put = |key: &str, value: Value| wire.push((key.to_owned(), value));
  put("sessionId", text(raw.get("session_id"), "unknown").into());
  put("cwd", text(raw.get("cwd"), &project).into());
  put("projectRoot", project.clone().into());
  put("worktree", project.into());
  put("toolCallId", text(raw.get("tool_use_id"), "unknown").into());
  put("toolName", tool.into());
  let command = input
    .get("command")
    .and_then(Value::as_str)
    .filter(|command| !command.is_empty())
    .map(str::to_owned);
  put("toolInput", input);
  let shell = matches!(tool, "Bash" | "Shell");
  wire.extend(typed(session.phase, command.filter(|_| shell), raw));
  wire
}

/// The `type` and its fields: `tool/post` with `toolOutput` (`tool_response ??
/// tool_output`, so a null response falls through), `shell/pre` with the
/// command of a `Bash` or `Shell` call, else `tool/pre`.
fn typed(
  phase: Phase,
  shell_command: Option<String>,
  raw: &Map<String, Value>,
) -> Vec<(String, Value)> {
  let field = |key: &str, value: Value| (key.to_owned(), value);
  match (phase, shell_command) {
    (Phase::Post, _) => {
      let output = match raw.get("tool_response") {
        Some(response) if !response.is_null() => Some(response),
        Some(_) | None => raw.get("tool_output"),
      };
      let mut fields = vec![field("type", "tool/post".into())];
      fields.extend(output.map(|output| field("toolOutput", output.clone())));
      fields
    }
    (Phase::Pre, Some(command)) => vec![
      field("type", "shell/pre".into()),
      field("command", command.into()),
    ],
    (Phase::Pre, None) => vec![field("type", "tool/pre".into())],
  }
}

#[cfg(test)]
#[path = "tests/event_test.rs"]
mod tests;
