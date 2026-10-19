//! Muse output: the exit-0 `{"decision":"block"}` line refuses a tool call, a
//! permission or a prompt and replaces a tool's result, and `systemMessage`
//! carries advice on every event. Muse rejects a `decision` on `SessionStart`
//! and `PreCompact`, which `normalize` already turns into advice there. This
//! writes no `hookSpecificOutput`: its only ask hangs a headless session, and
//! its `additionalContext` was never proven in a live run.

use super::hook::block;
use super::{Normal, Object};

/// The stdout for `normal`, which `normalize` already fitted to the event.
pub(super) fn output(normal: Normal<'_>) -> String {
  match normal {
    Normal::Allow => String::new(),
    // Muse never supports ask, so an ask here can only fail closed.
    Normal::Deny(reason) | Normal::Block(reason) | Normal::Ask(reason) => block(reason),
    Normal::Advisory(message) => Object::new().text("systemMessage", message).line(),
  }
}

#[cfg(test)]
#[path = "tests/muse_test.rs"]
mod tests;
