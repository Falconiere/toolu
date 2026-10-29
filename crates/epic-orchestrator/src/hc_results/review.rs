//! Handoff imported work to a local worker for the normal delivery pipeline.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_mux::hc::{Client, Launch, address, require_owner, status};
use toolu_runtime::env::Env;

use super::field;
use crate::hosts::{AgentOpts, HostKind, agent_args, host_kind};
use crate::launch::record::{Record, text, write_record};

fn error(error: &toolu_mux::hc::Error) -> String {
  format!("hc {}: {}", error.code, error.message)
}

fn command(record: &Record) -> Result<Vec<String>, String> {
  let kind = host_kind(field(record, "kind")?)?;
  let opts = AgentOpts {
    key: field(record, "key")?.to_owned(),
    model: text(record, "model").map(str::to_owned),
    effort: text(record, "effort").map(str::to_owned),
    bypass: record.get("bypass") == Some(&Value::Bool(true)),
    permission_mode: text(record, "permission_mode").unwrap_or("auto").to_owned(),
    ..AgentOpts::default()
  };
  let mut argv = vec![
    if kind == HostKind::Cursor {
      "cursor-agent"
    } else {
      kind.name()
    }
    .to_owned(),
  ];
  argv.extend(agent_args(kind, &opts)?);
  Ok(argv)
}

pub(super) fn handoff(path: &Path, record: &mut Record, env: &Env) -> Result<(), String> {
  prepare(path, record)?;
  if text(record, "review_prompt_state") == Some("acknowledged") {
    return Ok(());
  }
  let brief = field(record, "brief")?.to_owned();
  if !Path::new(&brief).is_file() {
    return Err("local review brief is missing".into());
  }
  let address = start(path, record, env)?;
  let client = Client::new(env);
  if text(record, "review_prompt_state") == Some("submitting") {
    return Err("review prompt delivery uncertain; inspect before retry".into());
  }
  let ready = client
    .call(
      &["wait", &address, "--timeout", "90"],
      Duration::from_secs(100),
    )
    .map_err(|err| error(&err))?;
  let terminal = client.get(&address).map_err(|err| error(&err))?;
  if !ready_to_send(&ready, &terminal) {
    return Err("local review worker is not idle; inspect before handoff".into());
  }
  record.insert("review_prompt_state".into(), json!("submitting"));
  write_record(path, record)?;
  let prompt = format!(
    "You are an epic worker. Read {brief} and follow it exactly. The implementation is already committed in this worktree; inspect it, then complete the required local gate, review, push, PR and babysit pipeline. Report status with the brief's command."
  );
  client.send(&address, &prompt).map_err(|err| error(&err))?;
  record.insert("review_prompt_state".into(), json!("acknowledged"));
  write_record(path, record)
}

fn ready_to_send(wait: &Value, terminal: &Value) -> bool {
  wait.get("status").and_then(Value::as_str) == Some("idle")
    && wait.get("ended").and_then(Value::as_bool) != Some(true)
    && status(terminal) == "idle"
    && terminal.get("state").and_then(Value::as_str) != Some("finished")
}

fn prepare(path: &Path, record: &mut Record) -> Result<(), String> {
  if text(record, "review_terminal_name").is_none() {
    let remote = field(record, "terminal_address")?.to_owned();
    let run = field(record, "hc_run")?;
    let name = format!("review-{}", run.get(..24).ok_or("invalid run identity")?);
    record.insert("remote_terminal_address".into(), json!(remote));
    record.insert("review_terminal_name".into(), json!(name));
    record.insert("review_prompt_state".into(), json!("pending"));
    write_record(path, record)?;
  }
  Ok(())
}

fn start(path: &Path, record: &mut Record, env: &Env) -> Result<String, String> {
  let name = field(record, "review_terminal_name")?.to_owned();
  let owner = field(record, "terminal_owner")?.to_owned();
  let cwd = field(record, "worktree")?.to_owned();
  let client = Client::new(env);
  let saved = if record.get("controller_review") == Some(&Value::Bool(true)) {
    text(record, "terminal_address").map(str::to_owned)
  } else {
    None
  };
  let terminal = match client.get(saved.as_deref().unwrap_or(&name)) {
    Ok(terminal) => terminal,
    Err(err) if err.code == "not_found" && saved.is_none() => client
      .launch(&Launch {
        name: &name,
        cwd: &cwd,
        owner: &owner,
        machine: None,
        argv: &command(record)?,
      })
      .map_err(|err| error(&err))?,
    Err(err) => return Err(error(&err)),
  };
  require_owner(&terminal, &owner).map_err(|err| error(&err))?;
  if terminal.get("cwd").and_then(Value::as_str) != Some(&cwd)
    || terminal.get("state").and_then(Value::as_str) == Some("finished")
  {
    return Err("local review terminal identity or state differs".into());
  }
  let address = address(&terminal).map_err(|err| error(&err))?.to_owned();
  if saved.as_ref().is_some_and(|expected| expected != &address) {
    return Err("local review terminal address changed".into());
  }
  for (key, value) in [
    ("terminal_address", address.clone()),
    ("agent", format!("hc:{address}")),
    ("pane_id", format!("hc:{address}")),
    ("workspace_id", format!("hc:{address}")),
  ] {
    record.insert(key.into(), json!(value));
  }
  record.insert("terminal_name".into(), json!(name));
  record.insert("controller_review".into(), json!(true));
  write_record(path, record)?;
  Ok(address)
}

#[cfg(test)]
#[path = "tests/review_test.rs"]
mod tests;
