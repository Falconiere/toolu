//! The `OpenCode` hook payload: the `(input, output)` pair the SDK passes a
//! plugin hook (`@opencode-ai/plugin@1.18.34`), as one object. `tool.execute.before`
//! has `input {tool, sessionID, callID}` and `output {args}`; `tool.execute.after`
//! adds `args` to its input.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::LenientString;

/// One `OpenCode` hook call.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct OpencodePayload {
  /// The hook's input.
  pub input: OpencodeInput,
  /// The hook's mutable output, for example `{args}`.
  pub output: Option<Value>,
  /// Every other key.
  #[serde(flatten)]
  pub rest: Map<String, Value>,
}

/// The input of an `OpenCode` hook.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct OpencodeInput {
  /// The host's tool name, for example `bash`.
  pub tool: LenientString,
  /// The session (`sessionID`).
  #[serde(rename = "sessionID")]
  pub session_id: LenientString,
  /// The tool call (`callID`).
  #[serde(rename = "callID")]
  pub call_id: LenientString,
  /// A `tool.execute.after` call's arguments.
  pub args: Option<Value>,
  /// Every other key.
  #[serde(flatten)]
  pub rest: Map<String, Value>,
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
