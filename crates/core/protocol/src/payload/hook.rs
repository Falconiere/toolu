//! The Claude Code and Codex hook payload: one shape, where Codex adds `turn_id`
//! and `model`.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::LenientString;

/// What Claude Code or Codex writes on a command hook's stdin.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct HookPayload {
  /// The host session.
  pub session_id: LenientString,
  /// The session transcript file.
  pub transcript_path: LenientString,
  /// The hook's working directory.
  pub cwd: LenientString,
  /// The native event name, for example `PreToolUse`.
  pub hook_event_name: LenientString,
  /// Claude Code's permission mode.
  pub permission_mode: LenientString,
  /// Codex's model.
  pub model: LenientString,
  /// Codex's turn.
  pub turn_id: LenientString,
  /// The tool of a tool event.
  pub tool_name: LenientString,
  /// The host's id for the tool call.
  pub tool_use_id: LenientString,
  /// What started a `PreCompact`: `manual` or `auto`.
  pub trigger: LenientString,
  /// A manual `PreCompact`'s instructions.
  pub custom_instructions: LenientString,
  /// Why a `SessionEnd` happened.
  pub reason: LenientString,
  /// The tool input; any JSON value.
  pub tool_input: Option<Value>,
  /// A `PostToolUse` result.
  pub tool_response: Option<Value>,
  /// The older name of `tool_response`.
  pub tool_output: Option<Value>,
  /// A `UserPromptSubmit` prompt; any JSON value.
  pub prompt: Option<Value>,
  /// A `SessionStart` source: `startup`, `resume`, `clear` or `compact`.
  pub source: Option<Value>,
  /// A legacy `SessionStart` source key toolu's session-start still reads.
  pub session_event: Option<Value>,
  /// A legacy `SessionStart` source key toolu's session-start still reads.
  pub event: Option<Value>,
  /// Every other key.
  #[serde(flatten)]
  pub rest: Map<String, Value>,
}

#[cfg(test)]
#[path = "tests/hook_test.rs"]
mod tests;
