//! The Cursor hook payload: the fields the conformance harness renders
//! (`tools/toolu-conformance/src/harness/fixtures.ts` `cursorStdin`), plus
//! `session_id`, `tool_output` and `prompt` from <https://cursor.com/docs/hooks>.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::LenientString;

/// What Cursor writes on a hook's stdin.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct CursorPayload {
  /// The conversation, on every event.
  pub conversation_id: LenientString,
  /// The native event name, for example `beforeShellExecution`.
  pub hook_event_name: LenientString,
  /// The working directory of a tool or shell event.
  pub cwd: LenientString,
  /// The session of `sessionStart` and `sessionEnd`.
  pub session_id: LenientString,
  /// The tool of a tool or MCP event.
  pub tool_name: LenientString,
  /// The host's id for a `preToolUse` call.
  pub tool_use_id: LenientString,
  /// The command line of a shell event.
  pub command: LenientString,
  /// The server of an MCP event.
  pub mcp_server_name: LenientString,
  /// The workspace roots.
  pub workspace_roots: Option<Value>,
  /// The tool input: an object, or a JSON string for MCP events.
  pub tool_input: Option<Value>,
  /// A `postToolUse` result, as a JSON string.
  pub tool_output: Option<Value>,
  /// Whether a shell command runs sandboxed.
  pub sandbox: Option<Value>,
  /// A `beforeSubmitPrompt` prompt.
  pub prompt: Option<Value>,
  /// Every other key.
  #[serde(flatten)]
  pub rest: Map<String, Value>,
}

#[cfg(test)]
#[path = "tests/cursor_test.rs"]
mod tests;
