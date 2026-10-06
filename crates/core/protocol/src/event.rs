//! Host events: the native names whose hook can prevent the action, the host
//! events toolu registers hooks for, and every normalized event type.

use serde::{Deserialize, Serialize};

/// Events whose hook can prevent the action: a missing, crashed or incompatible
/// binary must block them. The same set as `ENFORCING_EVENTS` in
/// `packages/toolu-core/src/launcher/launcher.ts`.
pub const ENFORCING_EVENTS: &[&str] = &["PreToolUse", "PermissionRequest"];

/// Whether a hook registered under `event` enforces (blocks on failure).
pub fn is_enforcing(event: &str) -> bool {
  ENFORCING_EVENTS.contains(&event)
}

/// A host event toolu registers hooks for, in canonical form (`HOST_EVENTS` in
/// `packages/toolu-core/src/host/host-events.ts`). `session/resume`,
/// `session/clear` and `compaction` arrive as the host's session-start event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostEvent {
  /// A session starts (`session/start`).
  SessionStart,
  /// A session ends (`session/unload`).
  SessionUnload,
  /// The user submits a prompt (`prompt`).
  Prompt,
  /// The host is about to compact the context (`pre_compact`).
  PreCompact,
  /// The host asks whether to allow an action (`permission/evaluate`).
  PermissionEvaluate,
  /// Before a tool call (`tool/pre`).
  ToolPre,
  /// Before a shell command (`shell/pre`).
  ShellPre,
  /// After a tool call (`tool/post`).
  ToolPost,
}

impl HostEvent {
  /// Every host event, in `HOST_EVENTS` order.
  pub const ALL: [HostEvent; 8] = [
    HostEvent::SessionStart,
    HostEvent::SessionUnload,
    HostEvent::Prompt,
    HostEvent::PreCompact,
    HostEvent::PermissionEvaluate,
    HostEvent::ToolPre,
    HostEvent::ShellPre,
    HostEvent::ToolPost,
  ];

  /// The canonical slug, for example `tool/pre`.
  pub fn slug(self) -> &'static str {
    match self {
      HostEvent::SessionStart => "session/start",
      HostEvent::SessionUnload => "session/unload",
      HostEvent::Prompt => "prompt",
      HostEvent::PreCompact => "pre_compact",
      HostEvent::PermissionEvaluate => "permission/evaluate",
      HostEvent::ToolPre => "tool/pre",
      HostEvent::ShellPre => "shell/pre",
      HostEvent::ToolPost => "tool/post",
    }
  }

  /// The host event a slug names; exact slugs only.
  pub fn from_slug(slug: &str) -> Option<HostEvent> {
    HostEvent::ALL
      .into_iter()
      .find(|event| event.slug() == slug)
  }
}

/// Every normalized event type (`BridgeEvent` in
/// `packages/toolu-core/src/events/events.ts`); its slug is the wire form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "&'static str", try_from = "String")]
pub enum EventKind {
  /// `session/start`.
  SessionStart,
  /// `session/resume`.
  SessionResume,
  /// `session/clear`.
  SessionClear,
  /// `session/unload`.
  SessionUnload,
  /// `prompt`.
  Prompt,
  /// `pre_compact`.
  PreCompact,
  /// `compaction`: the context was compacted.
  Compaction,
  /// `permission/evaluate`.
  PermissionEvaluate,
  /// `tool/pre`.
  ToolPre,
  /// `tool/post`.
  ToolPost,
  /// `shell/pre`.
  ShellPre,
}

impl EventKind {
  /// Every event type, in `BridgeEvent` order.
  pub const ALL: [EventKind; 11] = [
    EventKind::SessionStart,
    EventKind::SessionResume,
    EventKind::SessionClear,
    EventKind::SessionUnload,
    EventKind::Prompt,
    EventKind::PreCompact,
    EventKind::Compaction,
    EventKind::PermissionEvaluate,
    EventKind::ToolPre,
    EventKind::ToolPost,
    EventKind::ShellPre,
  ];

  /// The wire slug, for example `shell/pre`.
  pub fn slug(self) -> &'static str {
    match self {
      EventKind::SessionStart => "session/start",
      EventKind::SessionResume => "session/resume",
      EventKind::SessionClear => "session/clear",
      EventKind::SessionUnload => "session/unload",
      EventKind::Prompt => "prompt",
      EventKind::PreCompact => "pre_compact",
      EventKind::Compaction => "compaction",
      EventKind::PermissionEvaluate => "permission/evaluate",
      EventKind::ToolPre => "tool/pre",
      EventKind::ToolPost => "tool/post",
      EventKind::ShellPre => "shell/pre",
    }
  }
}

impl From<EventKind> for &'static str {
  fn from(kind: EventKind) -> &'static str {
    kind.slug()
  }
}

impl TryFrom<String> for EventKind {
  type Error = String;

  fn try_from(slug: String) -> Result<EventKind, String> {
    EventKind::ALL
      .into_iter()
      .find(|kind| kind.slug() == slug)
      .ok_or_else(|| format!("unknown event type `{slug}`"))
  }
}

#[cfg(test)]
#[path = "tests/event_test.rs"]
mod tests;
