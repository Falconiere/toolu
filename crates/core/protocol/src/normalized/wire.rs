//! The flat wire of [`NormalizedEvent`]: one strict struct for every type, and
//! a table of the optional fields each type may carry, so a field of another
//! type fails to parse as an unknown field would.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{NormalizedEvent, Session, Tool};
use crate::event::EventKind;
use crate::text::Text;

/// The fields of a tool event before the shell or post extras.
const TOOL: [&str; 3] = ["toolCallId", "toolName", "toolInput"];

/// Every field any type carries; `deny_unknown_fields` refuses the rest.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Wire {
  #[serde(rename = "type")]
  kind: EventKind,
  session_id: Text,
  cwd: Text,
  project_root: Text,
  worktree: Text,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  tool_call_id: Option<Text>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  tool_name: Option<Text>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  tool_input: Option<Map<String, Value>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  tool_output: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  command: Option<Text>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  permission: Option<Text>,
}

impl Wire {
  /// Which optional fields this document carries, by wire name.
  fn present(&self) -> [(&'static str, bool); 7] {
    [
      ("toolCallId", self.tool_call_id.is_some()),
      ("toolName", self.tool_name.is_some()),
      ("toolInput", self.tool_input.is_some()),
      ("toolOutput", self.tool_output.is_some()),
      ("command", self.command.is_some()),
      ("prompt", self.prompt.is_some()),
      ("permission", self.permission.is_some()),
    ]
  }
}

/// The optional fields `kind` may carry, by wire name.
fn allowed(kind: EventKind) -> &'static [&'static str] {
  match kind {
    EventKind::SessionStart
    | EventKind::SessionResume
    | EventKind::SessionClear
    | EventKind::SessionUnload
    | EventKind::PreCompact
    | EventKind::Compaction => &[],
    EventKind::Prompt => &["prompt"],
    EventKind::PermissionEvaluate => &["permission"],
    EventKind::ToolPre => &TOOL,
    EventKind::ToolPost => &["toolCallId", "toolName", "toolInput", "toolOutput"],
    EventKind::ShellPre => &["toolCallId", "toolName", "toolInput", "command"],
  }
}

/// `value`, or the error that `kind` needs `field`.
fn need<T>(value: Option<T>, field: &str, kind: EventKind) -> Result<T, String> {
  value.ok_or_else(|| format!("{} needs `{field}`", kind.slug()))
}

/// The optional fields of a document, before `kind` picks its variant.
#[derive(Default)]
struct Parts {
  call_id: Option<Text>,
  name: Option<Text>,
  input: Option<Map<String, Value>>,
  output: Option<Value>,
  command: Option<Text>,
  prompt: Option<String>,
  permission: Option<Text>,
}

impl Parts {
  fn set_tool(&mut self, tool: Tool) {
    self.call_id = Some(tool.call_id);
    self.name = Some(tool.name);
    self.input = Some(tool.input);
  }

  fn tool(&mut self, kind: EventKind) -> Result<Tool, String> {
    Ok(Tool {
      call_id: need(self.call_id.take(), "toolCallId", kind)?,
      name: need(self.name.take(), "toolName", kind)?,
      input: self.input.take().unwrap_or_default(),
    })
  }
}

impl Wire {
  fn into_parts(self) -> (Session, Parts) {
    let session = Session {
      session_id: self.session_id,
      cwd: self.cwd,
      project_root: self.project_root,
      worktree: self.worktree,
    };
    let parts = Parts {
      call_id: self.tool_call_id,
      name: self.tool_name,
      input: self.tool_input,
      output: self.tool_output,
      command: self.command,
      prompt: self.prompt,
      permission: self.permission,
    };
    (session, parts)
  }
}

/// The `kind` variant of `session` and `parts`; a required field may be missing.
fn assemble(
  kind: EventKind,
  session: Session,
  mut parts: Parts,
) -> Result<NormalizedEvent, String> {
  Ok(match kind {
    EventKind::SessionStart => NormalizedEvent::SessionStart(session),
    EventKind::SessionResume => NormalizedEvent::SessionResume(session),
    EventKind::SessionClear => NormalizedEvent::SessionClear(session),
    EventKind::SessionUnload => NormalizedEvent::SessionUnload(session),
    EventKind::PreCompact => NormalizedEvent::PreCompact(session),
    EventKind::Compaction => NormalizedEvent::Compaction(session),
    EventKind::Prompt => NormalizedEvent::Prompt {
      session,
      prompt: need(parts.prompt, "prompt", kind)?,
    },
    EventKind::PermissionEvaluate => NormalizedEvent::PermissionEvaluate {
      session,
      permission: need(parts.permission, "permission", kind)?,
    },
    EventKind::ToolPre => NormalizedEvent::ToolPre {
      session,
      tool: parts.tool(kind)?,
    },
    EventKind::ToolPost => NormalizedEvent::ToolPost {
      session,
      tool: parts.tool(kind)?,
      output: parts.output,
    },
    EventKind::ShellPre => NormalizedEvent::ShellPre {
      session,
      tool: parts.tool(kind)?,
      command: need(parts.command, "command", kind)?,
    },
  })
}

impl TryFrom<Wire> for NormalizedEvent {
  type Error = String;

  fn try_from(wire: Wire) -> Result<NormalizedEvent, String> {
    let kind = wire.kind;
    let foreign = wire
      .present()
      .into_iter()
      .find(|(field, here)| *here && !allowed(kind).contains(field));
    if let Some((field, _)) = foreign {
      return Err(format!("`{field}` is not a field of {}", kind.slug()));
    }
    let (session, parts) = wire.into_parts();
    assemble(kind, session, parts)
  }
}

/// Split `event` into its session and its type-specific fields.
fn split(event: NormalizedEvent) -> (Session, Parts) {
  let mut parts = Parts::default();
  let session = match event {
    NormalizedEvent::SessionStart(session)
    | NormalizedEvent::SessionResume(session)
    | NormalizedEvent::SessionClear(session)
    | NormalizedEvent::SessionUnload(session)
    | NormalizedEvent::PreCompact(session)
    | NormalizedEvent::Compaction(session) => session,
    NormalizedEvent::Prompt { session, prompt } => {
      parts.prompt = Some(prompt);
      session
    }
    NormalizedEvent::PermissionEvaluate {
      session,
      permission,
    } => {
      parts.permission = Some(permission);
      session
    }
    NormalizedEvent::ToolPre { session, tool } => {
      parts.set_tool(tool);
      session
    }
    NormalizedEvent::ToolPost {
      session,
      tool,
      output,
    } => {
      parts.set_tool(tool);
      parts.output = output;
      session
    }
    NormalizedEvent::ShellPre {
      session,
      tool,
      command,
    } => {
      parts.set_tool(tool);
      parts.command = Some(command);
      session
    }
  };
  (session, parts)
}

impl From<NormalizedEvent> for Wire {
  fn from(event: NormalizedEvent) -> Wire {
    let kind = event.kind();
    let (session, parts) = split(event);
    Wire {
      kind,
      session_id: session.session_id,
      cwd: session.cwd,
      project_root: session.project_root,
      worktree: session.worktree,
      tool_call_id: parts.call_id,
      tool_name: parts.name,
      tool_input: parts.input,
      tool_output: parts.output,
      command: parts.command,
      prompt: parts.prompt,
      permission: parts.permission,
    }
  }
}

#[cfg(test)]
#[path = "tests/wire_test.rs"]
mod tests;
