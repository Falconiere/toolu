//! Resume a fixer whose durable launch identity may precede its terminal.

use serde_json::{Value, json};
use toolu_mux::hc::{self, Client, Launch};

use super::{Dispatcher, Failure, Result, failure};

fn can_launch(group: &Value, error: &Failure) -> bool {
  error.extra.get("hcKind").and_then(Value::as_str) == Some("not_found")
    && group.get("hcAddress").and_then(Value::as_str).is_none()
    && group.get("hcSendStarted") != Some(&Value::Bool(true))
}

impl Dispatcher<'_> {
  /// Resume only a proven absent launch, never an uncertain send or lost address.
  pub(in crate::dispatch) fn recover_hc(&mut self, seq: u64, group: &Value) -> Result<()> {
    let terminal = match self.hc_terminal(group) {
      Ok(terminal) => terminal,
      Err(error) if can_launch(group, &error) => self.start_missing_hc(group)?,
      Err(error) => return Err(error),
    };
    let address = hc::address(&terminal)
      .map_err(|error| failure(&error))?
      .to_owned();
    self.patch_group(seq, &json!({"hcAddress": address}))?;
    if group.get("hcSendStarted") == Some(&Value::Bool(true)) {
      return self.block_hc(seq);
    }
    let brief = group
      .get("brief")
      .and_then(Value::as_str)
      .filter(|s| !s.is_empty())
      .ok_or_else(|| Failure::new("state_malformed", "missing fixer brief"))?;
    let message = format!("You are a pr-babysit fixer. Read {brief} and follow it exactly.");
    self.send_hc(seq, &address, &message)
  }

  fn start_missing_hc(&self, group: &Value) -> Result<Value> {
    let field = |key| group.get(key).and_then(Value::as_str).unwrap_or_default();
    let command = group
      .get("hcCommand")
      .and_then(Value::as_array)
      .and_then(|argv| {
        argv
          .iter()
          .map(|value| value.as_str().map(str::to_owned))
          .collect::<Option<Vec<_>>>()
      })
      .filter(|argv| argv.first().is_some_and(|s| !s.is_empty()))
      .ok_or_else(|| Failure::new("state_malformed", "missing saved fixer command"))?;
    let remote = self.remote();
    let machine = remote
      .and_then(|r| r.get("machine"))
      .and_then(Value::as_str);
    let cwd = remote
      .and_then(|r| r.get("path"))
      .and_then(Value::as_str)
      .or_else(|| self.worktree_field("path"))
      .unwrap_or_default();
    Client::new(self.env)
      .launch(&Launch {
        name: field("hcName"),
        owner: field("hcOwner"),
        cwd,
        machine,
        argv: &command,
      })
      .map_err(|error| failure(&error))
  }
}

#[cfg(test)]
#[path = "tests/recover_test.rs"]
mod tests;
