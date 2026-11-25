//! Each host's names for the host events (`packages/toolu-core/src/host/host-events.ts`):
//! Claude Code, Codex and Muse use `PascalCase`, Cursor `camelCase` and Hermes `snake_case`.
//! `OpenCode` hooks run under `toolu opencode bridge`, which hands every hook a
//! Claude-shaped payload, so `OpenCode` uses Claude's names; its own hook names
//! are mapped in `crate::opencode::event`.
//! `None` marks an event the host does not have.
//! Muse's `Notification`, `SubagentStop` and `PostToolUseFailure` have
//! no corresponding [`HostEvent`] and do not reverse-map.

use crate::event::HostEvent;
use crate::host::Host;

/// Cursor names that only map back: Cursor splits tool events by kind.
const CURSOR_ALIASES: [(&str, HostEvent); 4] = [
  ("beforeMCPExecution", HostEvent::ToolPre),
  ("afterFileEdit", HostEvent::ToolPost),
  ("afterShellExecution", HostEvent::ToolPost),
  ("afterMCPExecution", HostEvent::ToolPost),
];

/// `host`'s name for `event`, or `None` when the host has no such event.
pub fn native_event(host: Host, event: HostEvent) -> Option<&'static str> {
  match host {
    Host::Grok if event == HostEvent::PermissionEvaluate => None,
    Host::Codex | Host::Opencode if event == HostEvent::Stop => None,
    Host::Claude | Host::Muse | Host::Codex | Host::Opencode | Host::Grok | Host::Qwen => {
      Some(pascal(event))
    }
    Host::Cursor => cursor(event),
    Host::Hermes => hermes(event),
  }
}

/// The host event a native name stands for. A name two events share maps to
/// the first in [`HostEvent::ALL`]; host-specific aliases only map back.
pub fn canonical_event(host: Host, native: &str) -> Option<HostEvent> {
  if host == Host::Claude && native == "PostToolUseFailure" {
    return Some(HostEvent::ToolPost);
  }
  let row = HostEvent::ALL
    .into_iter()
    .find(|event| native_event(host, *event) == Some(native));
  if host != Host::Cursor {
    return row;
  }
  row.or_else(|| {
    CURSOR_ALIASES
      .into_iter()
      .find(|(alias, _)| *alias == native)
      .map(|(_, event)| event)
  })
}

/// Every host that uses `native`, in [`Host::ALL`] order.
pub fn hosts_for_native(native: &str) -> Vec<Host> {
  Host::ALL
    .into_iter()
    .filter(|host| canonical_event(*host, native).is_some())
    .collect()
}

fn pascal(event: HostEvent) -> &'static str {
  match event {
    HostEvent::SessionStart => "SessionStart",
    HostEvent::SessionUnload => "SessionEnd",
    HostEvent::Prompt => "UserPromptSubmit",
    HostEvent::PreCompact => "PreCompact",
    HostEvent::PermissionEvaluate => "PermissionRequest",
    HostEvent::ToolPre | HostEvent::ShellPre => "PreToolUse",
    HostEvent::ToolPost => "PostToolUse",
    HostEvent::Stop => "Stop",
  }
}

fn cursor(event: HostEvent) -> Option<&'static str> {
  match event {
    HostEvent::SessionStart => Some("sessionStart"),
    HostEvent::SessionUnload => Some("sessionEnd"),
    HostEvent::Prompt => Some("beforeSubmitPrompt"),
    HostEvent::PreCompact => Some("preCompact"),
    HostEvent::PermissionEvaluate => None,
    HostEvent::ToolPre => Some("preToolUse"),
    HostEvent::ShellPre => Some("beforeShellExecution"),
    HostEvent::ToolPost => Some("postToolUse"),
    HostEvent::Stop => Some("stop"),
  }
}

fn hermes(event: HostEvent) -> Option<&'static str> {
  match event {
    HostEvent::SessionStart => Some("on_session_start"),
    HostEvent::SessionUnload => Some("on_session_end"),
    HostEvent::Prompt => Some("pre_llm_call"),
    HostEvent::PreCompact | HostEvent::PermissionEvaluate | HostEvent::Stop => None,
    HostEvent::ToolPre | HostEvent::ShellPre => Some("pre_tool_call"),
    HostEvent::ToolPost => Some("post_tool_call"),
  }
}

#[cfg(test)]
#[path = "tests/native_test.rs"]
mod tests;
