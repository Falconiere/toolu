//! `NormalizedEvent`: the host-neutral event vocabulary
//! (`packages/toolu-core/src/events/events.ts`). Every host payload maps to one.
//!
//! The wire is TypeScript's flat camelCase object with a `type` slug. It is
//! strict, unlike zod's `z.object`: an unknown field, or a field another type
//! carries, fails to parse (`normalized/wire.rs`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::event::EventKind;
use crate::text::Text;

mod wire;

/// Where an event happened: every event carries these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
  /// The host's session id (`sessionId`).
  pub session_id: Text,
  /// The hook's working directory.
  pub cwd: Text,
  /// The project root (`projectRoot`).
  pub project_root: Text,
  /// The worktree.
  pub worktree: Text,
}

/// The tool call of a tool event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
  /// The host's id for the call (`toolCallId`).
  pub call_id: Text,
  /// The tool name (`toolName`).
  pub name: Text,
  /// The tool input (`toolInput`); absent reads as `{}`.
  pub input: Map<String, Value>,
}

/// One normalized host event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "wire::Wire", into = "wire::Wire")]
pub enum NormalizedEvent {
  /// `session/start`.
  SessionStart(Session),
  /// `session/resume`.
  SessionResume(Session),
  /// `session/clear`.
  SessionClear(Session),
  /// `session/unload`.
  SessionUnload(Session),
  /// `prompt`: the user submitted a prompt.
  Prompt {
    /// Where.
    session: Session,
    /// The prompt text; it may be empty.
    prompt: String,
  },
  /// `pre_compact`.
  PreCompact(Session),
  /// `compaction`: the context was compacted.
  Compaction(Session),
  /// `permission/evaluate`: the host asks whether to allow an action.
  PermissionEvaluate {
    /// Where.
    session: Session,
    /// What the host asks about.
    permission: Text,
  },
  /// `tool/pre`: before a tool call.
  ToolPre {
    /// Where.
    session: Session,
    /// The call.
    tool: Tool,
  },
  /// `tool/post`: after a tool call.
  ToolPost {
    /// Where.
    session: Session,
    /// The call.
    tool: Tool,
    /// The tool's result (`toolOutput`), when the host reported one.
    output: Option<Value>,
  },
  /// `shell/pre`: before a shell command.
  ShellPre {
    /// Where.
    session: Session,
    /// The call.
    tool: Tool,
    /// The command line.
    command: Text,
  },
}

impl NormalizedEvent {
  /// The event's type.
  pub fn kind(&self) -> EventKind {
    match self {
      NormalizedEvent::SessionStart(_) => EventKind::SessionStart,
      NormalizedEvent::SessionResume(_) => EventKind::SessionResume,
      NormalizedEvent::SessionClear(_) => EventKind::SessionClear,
      NormalizedEvent::SessionUnload(_) => EventKind::SessionUnload,
      NormalizedEvent::Prompt { .. } => EventKind::Prompt,
      NormalizedEvent::PreCompact(_) => EventKind::PreCompact,
      NormalizedEvent::Compaction(_) => EventKind::Compaction,
      NormalizedEvent::PermissionEvaluate { .. } => EventKind::PermissionEvaluate,
      NormalizedEvent::ToolPre { .. } => EventKind::ToolPre,
      NormalizedEvent::ToolPost { .. } => EventKind::ToolPost,
      NormalizedEvent::ShellPre { .. } => EventKind::ShellPre,
    }
  }

  /// Where the event happened.
  pub fn session(&self) -> &Session {
    match self {
      NormalizedEvent::SessionStart(session)
      | NormalizedEvent::SessionResume(session)
      | NormalizedEvent::SessionClear(session)
      | NormalizedEvent::SessionUnload(session)
      | NormalizedEvent::PreCompact(session)
      | NormalizedEvent::Compaction(session)
      | NormalizedEvent::Prompt { session, .. }
      | NormalizedEvent::PermissionEvaluate { session, .. }
      | NormalizedEvent::ToolPre { session, .. }
      | NormalizedEvent::ToolPost { session, .. }
      | NormalizedEvent::ShellPre { session, .. } => session,
    }
  }

  /// The tool call, for a tool event.
  pub fn tool(&self) -> Option<&Tool> {
    match self {
      NormalizedEvent::ToolPre { tool, .. }
      | NormalizedEvent::ToolPost { tool, .. }
      | NormalizedEvent::ShellPre { tool, .. } => Some(tool),
      NormalizedEvent::SessionStart(_)
      | NormalizedEvent::SessionResume(_)
      | NormalizedEvent::SessionClear(_)
      | NormalizedEvent::SessionUnload(_)
      | NormalizedEvent::PreCompact(_)
      | NormalizedEvent::Compaction(_)
      | NormalizedEvent::Prompt { .. }
      | NormalizedEvent::PermissionEvaluate { .. } => None,
    }
  }
}

#[cfg(test)]
#[path = "tests/normalized_test.rs"]
mod tests;
