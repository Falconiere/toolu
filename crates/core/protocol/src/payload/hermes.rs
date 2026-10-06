//! The Hermes shell-hook payload: only the documented top-level envelope
//! (<https://hermes-agent.nousresearch.com/docs/user-guide/features/hooks>).
//! No Hermes payload is captured in this repository, so `extra` stays opaque.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::LenientString;

/// What Hermes writes on a shell hook's stdin.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct HermesPayload {
  /// The native event name, for example `pre_tool_call`.
  pub hook_event_name: LenientString,
  /// The tool of a tool event.
  pub tool_name: LenientString,
  /// The session.
  pub session_id: LenientString,
  /// The working directory.
  pub cwd: LenientString,
  /// The tool arguments, when they are an object.
  pub tool_input: Option<Value>,
  /// Every other callback argument, as Hermes passes it.
  pub extra: Option<Value>,
  /// Every other key.
  #[serde(flatten)]
  pub rest: Map<String, Value>,
}

#[cfg(test)]
#[path = "tests/hermes_test.rs"]
mod tests;
