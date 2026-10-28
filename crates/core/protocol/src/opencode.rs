//! `OpenCode` V2 as a toolu host. `OpenCode` plugins are JavaScript modules, so each
//! installed toolu plugin carries a generated shim (`crate::launcher::opencode`)
//! that registers `OpenCode`'s hooks and hands every call to
//! `toolu opencode bridge`. The bridge reads the plugin's Claude-format
//! `hooks/hooks.json`, translates the call into the Claude-shaped payload every
//! toolu hook reads ([`event`]), runs the matching hooks and folds their output
//! into one answer for the shim ([`reply`]).

pub mod event;
pub mod reply;
mod tools;

/// One `OpenCode` hook the shim forwards, and the `hooks.json` event it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
  /// The first prompt or tool call of a session the shim has not seen.
  SessionStart,
  /// `session.hook("prompt")`.
  Prompt,
  /// `session.hook("compaction")`.
  Compaction,
  /// `tool.hook("execute.before")`.
  ToolBefore,
  /// `tool.hook("execute.after")`.
  ToolAfter,
}

impl Hook {
  /// Every hook, in the order the shim registers them.
  pub const ALL: [Hook; 5] = [
    Hook::SessionStart,
    Hook::Prompt,
    Hook::Compaction,
    Hook::ToolBefore,
    Hook::ToolAfter,
  ];

  /// The bridge's `--event` value.
  pub fn name(self) -> &'static str {
    match self {
      Hook::SessionStart => "session.start",
      Hook::Prompt => "session.prompt",
      Hook::Compaction => "session.compaction",
      Hook::ToolBefore => "tool.execute.before",
      Hook::ToolAfter => "tool.execute.after",
    }
  }

  /// The hook a `--event` value names.
  pub fn parse(text: &str) -> Option<Hook> {
    Hook::ALL.into_iter().find(|hook| hook.name() == text)
  }

  /// The `hooks.json` event whose entries this hook runs.
  pub fn claude_event(self) -> &'static str {
    match self {
      Hook::SessionStart => "SessionStart",
      Hook::Prompt => "UserPromptSubmit",
      Hook::Compaction => "PreCompact",
      Hook::ToolBefore => "PreToolUse",
      Hook::ToolAfter => "PostToolUse",
    }
  }

  /// The hook that runs the `hooks.json` event `name`, if `OpenCode` has one.
  pub fn for_claude_event(name: &str) -> Option<Hook> {
    Hook::ALL
      .into_iter()
      .find(|hook| hook.claude_event() == name)
  }

  /// Whether a failure refuses the action: only a tool call can be refused.
  pub fn enforcing(self) -> bool {
    self == Hook::ToolBefore
  }
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
