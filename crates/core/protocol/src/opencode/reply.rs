//! The bridge's answer to the shim: the Claude-format output of every hook that
//! ran, folded into one JSON line the shim acts on. Claude's hook contract
//! decides what each output means:
//!
//! - exit 2 refuses with stderr: a deny before a tool, a block after it, and
//!   context on an event that cannot refuse;
//! - exit 0 reads stdout: `hookSpecificOutput.permissionDecision` `deny` or
//!   `ask` and `decision: "block"` refuse, `additionalContext` is context for the
//!   model, `systemMessage` a message for the user, and plain text on
//!   `SessionStart` or `UserPromptSubmit` is context, and any other output before a
//!   tool fails closed, since a toolu hook prints a JSON object or nothing;
//! - any other status, or a timeout, fails closed before a tool and is reported
//!   on every other event, as the `hooks.json` launcher does.
//!
//! The line is `{"action": "continue" | "deny" | "block", "reason"?, "context"?,
//! "message"?}`: the shim throws on `deny`, appends `reason` and `context` to the
//! result of a tool call on `block` or with context, keeps session context in the
//! session's instructions and logs `message`.

use serde_json::{Map, Value};

use super::Hook;

/// What one hook left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran<'a> {
  /// How a failure names the hook (`launcher::hook_label`).
  pub label: &'a str,
  /// The exit status.
  pub exit_code: i32,
  /// Stdout.
  pub stdout: &'a str,
  /// Stderr.
  pub stderr: &'a str,
  /// The deadline passed and the hook was stopped.
  pub timed_out: bool,
}

/// The folded answer of one bridge call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reply {
  deny: Vec<String>,
  block: Vec<String>,
  context: Vec<String>,
  message: Vec<String>,
}

impl Reply {
  /// Whether a hook refused the tool call; later hooks need not run.
  pub fn denied(&self) -> bool {
    !self.deny.is_empty()
  }

  /// Refuse the call: a deny before a tool, a block after it, else context.
  pub fn refuse(&mut self, hook: Hook, reason: String) {
    match hook {
      Hook::ToolBefore => self.deny.push(reason),
      Hook::ToolAfter => self.block.push(reason),
      Hook::SessionStart | Hook::Prompt | Hook::Compaction => self.context.push(reason),
    }
  }

  /// A failure of the bridge or of a hook: a deny on an enforcing hook, else a message.
  pub fn fail(&mut self, hook: Hook, text: String) {
    if hook.enforcing() {
      self.deny.push(text);
    } else {
      self.message.push(text);
    }
  }

  /// Fold the output of one hook of `hook`.
  pub fn add(&mut self, hook: Hook, ran: &Ran<'_>) {
    if ran.timed_out {
      self.fail(hook, format!("{} timed out", ran.label));
      return;
    }
    match ran.exit_code {
      0 => self.stdout(hook, ran),
      2 => {
        let reason = stripped(ran.stderr);
        let reason = if reason.is_empty() {
          format!("{} refused without a reason", ran.label)
        } else {
          reason.to_owned()
        };
        self.refuse(hook, reason);
      }
      status => {
        let detail = stripped(ran.stderr);
        let tail = if detail.is_empty() {
          String::new()
        } else {
          format!(": {detail}")
        };
        self.fail(
          hook,
          format!("{} ended with status {status}{tail}", ran.label),
        );
      }
    }
  }

  fn stdout(&mut self, hook: Hook, ran: &Ran<'_>) {
    let text = ran.stdout.trim();
    if text.is_empty() {
      return;
    }
    let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(text) else {
      match hook {
        Hook::SessionStart | Hook::Prompt => self.context.push(text.to_owned()),
        // A hook that meant to refuse and printed something else must not let the tool run.
        Hook::ToolBefore => self.fail(
          hook,
          format!("{} printed output that is not a JSON object", ran.label),
        ),
        Hook::ToolAfter | Hook::Compaction => {}
      }
      return;
    };
    if hook == Hook::ToolBefore
      && let Some(problem) = malformed(&doc)
    {
      self.fail(hook, format!("{} printed {problem}", ran.label));
      return;
    }
    let specific = doc.get("hookSpecificOutput").and_then(Value::as_object);
    if let Some(reason) = refusal(&doc, specific) {
      self.refuse(hook, reason);
    }
    let context = specific.and_then(|specific| field(specific, "additionalContext"));
    self.context.extend(context);
    self.message.extend(field(&doc, "systemMessage"));
  }

  /// The JSON line the shim reads.
  pub fn json(&self) -> String {
    let action = if self.denied() {
      "deny"
    } else if self.block.is_empty() {
      "continue"
    } else {
      "block"
    };
    let mut out = Map::new();
    out.insert("action".to_owned(), action.into());
    let reasons = if self.denied() {
      &self.deny
    } else {
      &self.block
    };
    for (key, lines) in [
      ("reason", reasons),
      ("context", &self.context),
      ("message", &self.message),
    ] {
      if !lines.is_empty() {
        out.insert(key.to_owned(), lines.join("\n\n").into());
      }
    }
    Value::Object(out).to_string()
  }
}

/// A refusal's reason: `permissionDecision` `deny` or `ask`, `decision: "block"`
/// or `continue: false`.
fn refusal(doc: &Map<String, Value>, specific: Option<&Map<String, Value>>) -> Option<String> {
  let decision = specific.and_then(|specific| specific.get("permissionDecision"));
  if matches!(decision.and_then(Value::as_str), Some("deny" | "ask")) {
    let reason = specific.and_then(|specific| field(specific, "permissionDecisionReason"));
    return Some(reason.unwrap_or_else(|| "denied by a toolu hook".to_owned()));
  }
  if doc.get("decision").and_then(Value::as_str) == Some("block") {
    return Some(field(doc, "reason").unwrap_or_else(|| "blocked by a toolu hook".to_owned()));
  }
  if doc.get("continue").and_then(Value::as_bool) == Some(false) {
    return Some(field(doc, "stopReason").unwrap_or_else(|| "stopped by a toolu hook".to_owned()));
  }
  None
}

/// What makes a decision object unreadable: a `hookSpecificOutput` that is not an
/// object, or a `permissionDecision` or `decision` that is not a string. Before a
/// tool such output fails closed rather than reading as no decision.
fn malformed(doc: &Map<String, Value>) -> Option<&'static str> {
  let specific = doc.get("hookSpecificOutput");
  if specific.is_some_and(|value| !value.is_object()) {
    return Some("a hookSpecificOutput that is not an object");
  }
  let decision = specific.and_then(|specific| specific.get("permissionDecision"));
  if decision.is_some_and(|value| !value.is_string()) {
    return Some("a permissionDecision that is not a string");
  }
  if doc.get("decision").is_some_and(|value| !value.is_string()) {
    return Some("a decision that is not a string");
  }
  None
}

/// A non-empty string field.
fn field(doc: &Map<String, Value>, key: &str) -> Option<String> {
  doc
    .get(key)
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|text| !text.is_empty())
    .map(str::to_owned)
}

/// Stderr without surrounding space and the launcher's `blocked: ` prefix.
fn stripped(stderr: &str) -> &str {
  let text = stderr.trim();
  text.strip_prefix("blocked: ").unwrap_or(text).trim()
}

#[cfg(test)]
#[path = "tests/reply_test.rs"]
mod tests;
