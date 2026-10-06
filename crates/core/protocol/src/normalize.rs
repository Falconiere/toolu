//! Map a host [`Payload`] to its [`NormalizedEvent`]: `toolEvent` in
//! `packages/toolu-core/src/dispatch/dispatch-context.ts`, extended to the
//! session events with the `source` chain of `plugins/toolu/hooks/src/session-start.ts`.
//! `OpenCode` payloads need the `OpenCode` tool-name map (#462) and map to nothing yet.

use serde_json::{Map, Value};

use crate::event::HostEvent;
use crate::normalized::{NormalizedEvent, Session, Tool};
use crate::payload::{CursorPayload, HermesPayload, HookPayload, Payload};
use crate::text::Text;

/// The roots the caller resolved (`toolu-runtime`): every event carries them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
  /// The project root.
  pub project_root: Text,
  /// The worktree.
  pub worktree: Text,
}

/// What one payload says, read the same way for every host.
#[derive(Default)]
struct View<'a> {
  session_id: Option<&'a str>,
  cwd: Option<&'a str>,
  call_id: Option<&'a str>,
  tool_name: Option<&'a str>,
  tool_input: Map<String, Value>,
  tool_output: Option<&'a Value>,
  prompt: Option<&'a Value>,
  /// `source`, `session_event` and `event`, in jq `//` order.
  start: [Option<&'a Value>; 3],
}

impl Payload {
  /// The normalized form of this payload for `event`, or `None` for `OpenCode`.
  pub fn normalize(&self, event: HostEvent, roots: &Roots) -> Option<NormalizedEvent> {
    let view = match self {
      Payload::Claude(payload) | Payload::Codex(payload) => hook_view(payload),
      Payload::Cursor(payload) => cursor_view(payload),
      Payload::Hermes(payload) => hermes_view(payload),
      Payload::Opencode(_) => return None,
    };
    view.event(event, roots)
  }
}

impl View<'_> {
  fn event(self, event: HostEvent, roots: &Roots) -> Option<NormalizedEvent> {
    let session = Session {
      session_id: text_or(self.session_id, "unknown")?,
      cwd: self
        .cwd
        .and_then(|cwd| Text::new(cwd).ok())
        .unwrap_or_else(|| roots.project_root.clone()),
      project_root: roots.project_root.clone(),
      worktree: roots.worktree.clone(),
    };
    Some(match event {
      HostEvent::SessionStart => start(session, self.start),
      HostEvent::SessionUnload => NormalizedEvent::SessionUnload(session),
      HostEvent::Prompt => NormalizedEvent::Prompt {
        session,
        prompt: prompt_text(self.prompt),
      },
      HostEvent::PreCompact => NormalizedEvent::PreCompact(session),
      HostEvent::PermissionEvaluate => NormalizedEvent::PermissionEvaluate {
        session,
        permission: text_or(self.tool_name, "unknown")?,
      },
      HostEvent::ToolPre | HostEvent::ShellPre => pre_tool(session, self.tool()?),
      HostEvent::ToolPost => NormalizedEvent::ToolPost {
        session,
        output: self.tool_output.cloned(),
        tool: self.tool()?,
      },
    })
  }

  fn tool(&self) -> Option<Tool> {
    Some(Tool {
      call_id: text_or(self.call_id, "unknown")?,
      name: text_or(self.tool_name, "unknown")?,
      input: self.tool_input.clone(),
    })
  }
}

/// `value` when it is a non-empty string, else `fallback`; `None` only for an empty `fallback`.
fn text_or(value: Option<&str>, fallback: &str) -> Option<Text> {
  Text::new(value.filter(|text| !text.is_empty()).unwrap_or(fallback)).ok()
}

/// `shell/pre` for a `Bash` or `Shell` call with a non-empty string command, else `tool/pre`.
fn pre_tool(session: Session, tool: Tool) -> NormalizedEvent {
  let shell = matches!(tool.name.as_str(), "Bash" | "Shell");
  let command = tool
    .input
    .get("command")
    .and_then(Value::as_str)
    .and_then(|command| Text::new(command).ok());
  match command {
    Some(command) if shell => NormalizedEvent::ShellPre {
      session,
      tool,
      command,
    },
    Some(_) | None => NormalizedEvent::ToolPre { session, tool },
  }
}

/// The first of `chain` that is neither null nor false picks the session event.
fn start(session: Session, chain: [Option<&Value>; 3]) -> NormalizedEvent {
  let picked = chain
    .into_iter()
    .flatten()
    .find(|value| !matches!(value, Value::Null | Value::Bool(false)));
  match picked.and_then(Value::as_str) {
    Some("resume") => NormalizedEvent::SessionResume(session),
    Some("clear") => NormalizedEvent::SessionClear(session),
    Some("compact") => NormalizedEvent::Compaction(session),
    Some(_) | None => NormalizedEvent::SessionStart(session),
  }
}

/// A string prompt as itself; null, false or absent as `""`; any other value as compact JSON.
fn prompt_text(prompt: Option<&Value>) -> String {
  match prompt {
    None | Some(Value::Null | Value::Bool(false)) => String::new(),
    Some(Value::String(text)) => text.clone(),
    Some(other) => other.to_string(),
  }
}

/// `value` when it is an object, else `{}`.
fn object(value: Option<&Value>) -> Map<String, Value> {
  value
    .and_then(Value::as_object)
    .cloned()
    .unwrap_or_default()
}

fn hook_view(payload: &HookPayload) -> View<'_> {
  View {
    session_id: payload.session_id.text(),
    cwd: payload.cwd.text(),
    call_id: payload.tool_use_id.text(),
    tool_name: payload.tool_name.text(),
    tool_input: object(payload.tool_input.as_ref()),
    tool_output: payload
      .tool_response
      .as_ref()
      .or(payload.tool_output.as_ref()),
    prompt: payload.prompt.as_ref(),
    start: [
      payload.source.as_ref(),
      payload.session_event.as_ref(),
      payload.event.as_ref(),
    ],
  }
}

/// Cursor names the session `session_id` or `conversation_id`, sends a shell
/// command at the top level, and sends MCP input as a JSON string.
fn cursor_view(payload: &CursorPayload) -> View<'_> {
  let command = payload.command.text();
  let input = match (&payload.tool_input, command) {
    (Some(Value::String(json)), _) => serde_json::from_str(json).ok(),
    (Some(value), _) => value.as_object().cloned(),
    (None, Some(command)) => Some(Map::from_iter([(
      "command".to_owned(),
      Value::from(command),
    )])),
    (None, None) => None,
  };
  View {
    session_id: payload.session_id.text().or(payload.conversation_id.text()),
    cwd: payload.cwd.text(),
    call_id: payload.tool_use_id.text(),
    tool_name: payload.tool_name.text().or(command.map(|_| "Shell")),
    tool_input: input.unwrap_or_default(),
    tool_output: payload.tool_output.as_ref(),
    prompt: payload.prompt.as_ref(),
    start: [None; 3],
  }
}

fn hermes_view(payload: &HermesPayload) -> View<'_> {
  View {
    session_id: payload.session_id.text(),
    cwd: payload.cwd.text(),
    tool_name: payload.tool_name.text(),
    tool_input: object(payload.tool_input.as_ref()),
    ..View::default()
  }
}

#[cfg(test)]
#[path = "tests/normalize_test.rs"]
mod tests;
