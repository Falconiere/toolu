//! The event and context one walk hands its modules (`toolEvent` and
//! `toolContext` in `dispatch-context.ts`): a `NormalizedEvent` and
//! `RuleContext` for gates and rules, and the same in TypeScript key order for
//! the Bun bridge.

use serde_json::{Map, Value, json};
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Payload {
  pub(crate) text: String,
  /// `text` as `JSON.parse` reads it, keys in JavaScript order; `None` when it is not JSON.
  pub(crate) doc: Option<Ordered>,
  /// The walk's tool name: `"Edit"` for a split path, `""` when stdin is not JSON.
  pub(crate) tool_name: String,
  pub(crate) edit: Option<EditFields>,
}

/// What one walk's modules see.
pub(crate) struct View {
  pub(crate) event: NormalizedEvent,
  pub(crate) raw: Map<String, Value>,
  /// The event's fields in `toolEvent`'s key order, for the Bun bridge.
  fields: Vec<(String, Value)>,
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
  /// The event and context for `payload`.
  ///
  /// # Errors
  /// When the event does not form, which the fallbacks rule out.
  pub(crate) fn of(payload: &Payload, session: &Session<'_>) -> Result<View, String> {
    let raw = match payload.doc.as_ref().map(plain) {
      Some(Value::Object(map)) => map,
      Some(_) | None => Map::new(),
    };
    let fields = fields(payload, session, &raw);
    let wire = Value::Object(fields.iter().cloned().collect());
    let event: NormalizedEvent = serde_json::from_value(wire).map_err(|err| err.to_string())?;
    Ok(View { event, raw, fields })
  }

  /// The event as the Bun bridge sends it: `toolEvent`'s key order, with
  /// `toolInput` and `toolOutput` in the payload's own key order.
  pub(crate) fn ordered_event(&self, payload: &Payload) -> Ordered {
    let mut event = Ordered::Object(
      self
        .fields
        .iter()
        .map(|(key, value)| (key.clone(), Ordered::from(value)))
        .collect(),
    );
    let doc = payload.doc.as_ref();
    let field = |key: &str| doc.and_then(|doc| doc.get(key));
    let input = field("tool_input").filter(|input| matches!(input, Ordered::Object(_)));
    event.set(
      "toolInput",
      input.cloned().unwrap_or(Ordered::Object(Vec::new())),
    );
    if self.fields.iter().any(|(key, _)| key == "toolOutput") {
      let output = field("tool_response")
        .filter(|response| **response != Ordered::Null)
        .or_else(|| field("tool_output"));
      event.set("toolOutput", output.cloned().unwrap_or(Ordered::Null));
    }
    event
  }

  /// The context a gate or rule sees for this view.
  pub(crate) fn rule_context<'v>(
    &'v self,
    payload: &'v Payload,
    session: &'v Session<'_>,
  ) -> RuleContext<'v> {
    RuleContext {
      host: session.host,
      env: &session.env,
      config_root: &session.config_root,
      project_root: &session.project_root,
      cwd: Some(session.cwd()),
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
  payload: &Payload,
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
    payload.tool_name.as_str()
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

/// `toolContext` for the bridge, without `env`, which the runner takes from its process.
pub(crate) fn ordered_ctx(payload: &Payload, session: &Session<'_>) -> Ordered {
  let raw = match &payload.doc {
    Some(doc @ Ordered::Object(_)) => doc.clone(),
    Some(_) | None => Ordered::Object(Vec::new()),
  };
  let path = |path: &std::path::Path| Ordered::String(path.to_string_lossy().into_owned());
  let mut ctx = vec![
    (
      "host".to_owned(),
      Ordered::String(session.host.name().to_owned()),
    ),
    ("configRoot".to_owned(), path(&session.config_root)),
    ("projectRoot".to_owned(), path(&session.project_root)),
    ("cwd".to_owned(), path(session.cwd())),
    ("raw".to_owned(), raw),
  ];
  if let Some(edit) = &payload.edit {
    let fields = vec![
      (
        "operation".to_owned(),
        Ordered::String(edit.operation.name().to_owned()),
      ),
      ("from".to_owned(), Ordered::String(edit.from.clone())),
      ("movedTo".to_owned(), Ordered::String(edit.moved_to.clone())),
    ];
    ctx.push(("edit".to_owned(), Ordered::Object(fields)));
  }
  Ordered::Object(ctx)
}

#[cfg(test)]
#[path = "tests/event_test.rs"]
mod tests;
