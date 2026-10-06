//! Each host's names for the host events (`packages/toolu-core/src/host/host-events.ts`):
//! Claude Code and Codex use `PascalCase`, Cursor `camelCase`, Hermes `snake_case` and
//! `OpenCode` dotted plugin hooks. `None` marks an event the host does not have.

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
    Host::Claude | Host::Codex => Some(pascal(event)),
    Host::Cursor => cursor(event),
    Host::Hermes => hermes(event),
    Host::Opencode => opencode(event),
  }
}

/// The host event a native name stands for. A name two events share maps to
/// the first in [`HostEvent::ALL`]; Cursor's aliases map last.
pub fn canonical_event(host: Host, native: &str) -> Option<HostEvent> {
  let row = HostEvent::ALL
    .into_iter()
    .find(|event| native_event(host, *event) == Some(native));
  row.or_else(|| match host {
    Host::Cursor => CURSOR_ALIASES
      .into_iter()
      .find(|(alias, _)| *alias == native)
      .map(|(_, event)| event),
    Host::Claude | Host::Codex | Host::Hermes | Host::Opencode => None,
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
  }
}

fn hermes(event: HostEvent) -> Option<&'static str> {
  match event {
    HostEvent::SessionStart => Some("on_session_start"),
    HostEvent::SessionUnload => Some("on_session_end"),
    HostEvent::Prompt => Some("pre_llm_call"),
    HostEvent::PreCompact | HostEvent::PermissionEvaluate => None,
    HostEvent::ToolPre | HostEvent::ShellPre => Some("pre_tool_call"),
    HostEvent::ToolPost => Some("post_tool_call"),
  }
}

fn opencode(event: HostEvent) -> Option<&'static str> {
  match event {
    HostEvent::SessionStart => Some("session.created"),
    HostEvent::SessionUnload => Some("session.deleted"),
    HostEvent::Prompt => Some("chat.message"),
    HostEvent::PreCompact => Some("experimental.session.compacting"),
    HostEvent::PermissionEvaluate => None,
    HostEvent::ToolPre | HostEvent::ShellPre => Some("tool.execute.before"),
    HostEvent::ToolPost => Some("tool.execute.after"),
  }
}

#[cfg(test)]
#[path = "tests/native_test.rs"]
mod tests;
