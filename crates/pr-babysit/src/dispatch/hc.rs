//! Local herdr-core terminals owned by a fixer attempt.

use std::time::Duration;

use serde_json::{Value, json};
use toolu_mux::hc::{self, Client, Launch};

use super::dispatcher::Dispatcher;
use super::launch::Agent;
use crate::error::{Failure, Result};

fn failure(error: &hc::Error) -> Failure {
  Failure::new("hc_error", format!("hc {}: {}", error.code, error.message))
    .with("uncertain", error.uncertain)
}

impl Dispatcher<'_> {
  pub(super) fn hc_backend(&self) -> bool {
    if let Some(backend) = self.worktree_field("multiplexer") {
      return backend == "herdr-app";
    }
    if self
      .state
      .get("herdrWorktree")
      .is_some_and(Value::is_object)
    {
      return false;
    }
    let roots = toolu_runtime::host::roots::Roots::new(self.env.clone(), None);
    let config = toolu_runtime::config::load::load(&roots, self.file.parent());
    let configured = self
      .env
      .get("TOOLU_EPIC_MULTIPLEXER")
      .or_else(|| config.data.get("multiplexer").and_then(Value::as_str))
      .unwrap_or("auto");
    configured == "herdr-app" || (configured == "auto" && self.env.get("HC_TERMINAL_ID").is_some())
  }

  pub(super) fn launch_hc(&mut self, seq: u64, agent: &Agent<'_>) -> Result<()> {
    let name = agent.name;
    let digest = crate::sha1::hex(format!("{}:{name}", self.file.display()).as_bytes());
    let short: String = digest.chars().take(24).collect();
    let terminal_name = format!("pb-{short}");
    let owner = format!("toolu.owner=pb-{digest}");
    let cwd = self
      .remote()
      .and_then(|remote| remote.get("path"))
      .and_then(Value::as_str)
      .or_else(|| self.worktree_field("path"))
      .unwrap_or_default()
      .to_owned();
    let machine = self
      .remote()
      .and_then(|remote| remote.get("machine"))
      .and_then(Value::as_str)
      .map(str::to_owned);
    let session = self.env.get("HC_SESSION").unwrap_or("default");
    self.patch_group(
      seq,
      &json!({"hcName": terminal_name, "hcOwner": owner, "hcSession": session}),
    )?;
    let mut command = vec![agent.host.cli().to_owned()];
    command.extend_from_slice(agent.args);
    if self.dry {
      return Ok(());
    }
    let client = Client::new(self.env);
    let terminal = client
      .launch(&Launch {
        name: &terminal_name,
        cwd: &cwd,
        owner: &owner,
        machine: machine.as_deref(),
        argv: &command,
      })
      .map_err(|error| failure(&error))?;
    let address = hc::address(&terminal)
      .map_err(|error| failure(&error))?
      .to_owned();
    let verified = client.get(&address).map_err(|error| failure(&error))?;
    hc::require_owner(&verified, &owner).map_err(|error| failure(&error))?;
    self.patch_group(seq, &json!({"hcAddress": address}))?;
    self.send_hc(seq, &address, agent.message)
  }

  fn send_hc(&mut self, seq: u64, address: &str, message: &str) -> Result<()> {
    let client = Client::new(self.env);
    client
      .call(
        &["wait", address, "--timeout", "90"],
        Duration::from_secs(100),
      )
      .map_err(|error| failure(&error))?;
    let ready = client.get(address).map_err(|error| failure(&error))?;
    if hc::status(&ready) != "idle" {
      return self.block_hc(seq);
    }
    // Persist the send intent before sending: an uncertain reply must never be resent.
    self.patch_group(seq, &json!({"hcSendStarted": true}))?;
    Client::new(self.env)
      .send(address, message)
      .map_err(|error| failure(&error))?;
    self.patch_group(seq, &json!({"status": "running", "hcSendConfirmed": true}))
  }
}

impl Dispatcher<'_> {
  pub(super) fn recover_hc(&mut self, seq: u64, group: &Value) -> Result<()> {
    let terminal = self.hc_terminal(group)?;
    let address = hc::address(&terminal)
      .map_err(|error| failure(&error))?
      .to_owned();
    self.patch_group(seq, &json!({"hcAddress": address}))?;
    if group.get("hcSendStarted") == Some(&Value::Bool(true)) || hc::status(&terminal) != "idle" {
      return self.block_hc(seq);
    }
    let brief = group
      .get("brief")
      .and_then(Value::as_str)
      .unwrap_or_default();
    if brief.is_empty() {
      return Err(Failure::new("state_malformed", "missing fixer brief"));
    }
    let message = format!("You are a pr-babysit fixer. Read {brief} and follow it exactly.");
    self.patch_group(seq, &json!({"hcSendStarted": true}))?;
    Client::new(self.env)
      .send(&address, &message)
      .map_err(|error| failure(&error))?;
    self.patch_group(seq, &json!({"status": "running", "hcSendConfirmed": true}))
  }

  pub(super) fn block_hc(&mut self, seq: u64) -> Result<()> {
    let blocked = json!({"status": "blocked", "reason": "agent_blocked"});
    self.patch_fixer(&blocked)?;
    self.patch_group(seq, &blocked)
  }

  pub(super) fn hc_terminal(&self, group: &Value) -> Result<Value> {
    let saved = group.get("hcAddress").and_then(Value::as_str);
    let name = group
      .get("hcName")
      .and_then(Value::as_str)
      .unwrap_or_default();
    let qualified = self
      .remote()
      .and_then(|remote| remote.get("machine"))
      .and_then(Value::as_str)
      .map_or_else(|| name.to_owned(), |machine| format!("{machine}/{name}"));
    let address = saved.unwrap_or(&qualified);
    let owner = group
      .get("hcOwner")
      .and_then(Value::as_str)
      .unwrap_or_default();
    let session = group
      .get("hcSession")
      .and_then(Value::as_str)
      .unwrap_or_default();
    if address.is_empty()
      || owner.is_empty()
      || session != self.env.get("HC_SESSION").unwrap_or("default")
    {
      return Err(Failure::new(
        "hc_error",
        "missing terminal identity or hc session differs",
      ));
    }
    let mut terminal = Client::new(self.env)
      .get(address)
      .map_err(|error| failure(&error))?;
    hc::require_owner(&terminal, owner).map_err(|error| failure(&error))?;
    let machine = self
      .remote()
      .and_then(|remote| remote.get("machine"))
      .and_then(Value::as_str);
    let immutable = immutable_address(&terminal, saved, machine)?;
    if let Some(fields) = terminal.as_object_mut() {
      fields.insert("address".into(), json!(immutable));
    }
    Ok(terminal)
  }

  pub(super) fn stop_hc(&self, group: &Value) -> Result<()> {
    let terminal = self.hc_terminal(group)?;
    let address = hc::address(&terminal).map_err(|error| failure(&error))?;
    let owner = group
      .get("hcOwner")
      .and_then(Value::as_str)
      .unwrap_or_default();
    if terminal.get("state").and_then(Value::as_str) != Some("finished") {
      Client::new(self.env)
        .kill_owned(address, owner)
        .map_err(|error| failure(&error))?;
    }
    Ok(())
  }

  pub(super) fn wait_hc(&self, group: &Value, remaining: u64) -> Result<Option<String>> {
    let terminal = self.hc_terminal(group)?;
    let address = hc::address(&terminal).map_err(|error| failure(&error))?;
    let timeout = remaining.to_string();
    Client::new(self.env)
      .call(
        &["wait", address, "--timeout", &timeout],
        Duration::from_secs(remaining + 30),
      )
      .map_err(|error| failure(&error))?;
    let result = Client::new(self.env)
      .get(address)
      .map_err(|error| failure(&error))?;
    let status = hc::status(&result);
    Ok(if matches!(status, "working" | "unknown") {
      None
    } else {
      Some(status.to_owned())
    })
  }

  pub(super) fn screen_hc(&self, group: &Value) -> Result<String> {
    let terminal = self.hc_terminal(group)?;
    let address = hc::address(&terminal).map_err(|error| failure(&error))?;
    let value = Client::new(self.env)
      .call(&["read", address, "--lines", "40"], Duration::from_secs(30))
      .map_err(|error| failure(&error))?;
    Ok(
      value
        .get("text")
        .or_else(|| value.get("screen"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned(),
    )
  }
}

fn immutable_address(
  terminal: &Value,
  saved: Option<&str>,
  machine: Option<&str>,
) -> Result<String> {
  let reported = hc::address(terminal).map_err(|error| failure(&error))?;
  let immutable = match machine {
    Some(machine) if !reported.contains('/') => format!("{machine}/{reported}"),
    Some(machine) if !reported.starts_with(&format!("{machine}/")) => {
      return Err(Failure::new("hc_error", "terminal machine differs"));
    }
    Some(_) | None => reported.to_owned(),
  };
  if saved.is_some_and(|value| value != immutable) {
    return Err(Failure::new("hc_error", "terminal address differs"));
  }
  Ok(immutable)
}

#[cfg(test)]
#[path = "tests/hc_test.rs"]
mod tests;
