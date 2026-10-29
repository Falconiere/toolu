//! Route saved `hc:` identities independently of the controller's configuration.

mod identity;

use crate::herdr::Call;
use crate::paths::Paths;
use serde_json::{Value, json};
use std::time::Duration;
use toolu_mux::Backend;
use toolu_mux::hc::{Client, status};
use toolu_runtime::env::Env;

/// A compatibility reply for lifecycle operations on a persisted hc terminal.
pub(crate) fn execute(
  paths: &Paths,
  env: &Env,
  backend: Backend,
  call: &Call,
) -> Option<Result<String, String>> {
  let client = Client::new(env);
  let result = match call {
    Call::AgentList if backend == Backend::HerdrApp => {
      hydrate(paths, env, &client).map(|listed| snapshot(&listed))
    }
    Call::Read { target } if target.starts_with("hc:") => {
      identity::verify(paths, env, hc_target(target))
        .and_then(|()| client.call(&["read", hc_target(target)], Duration::from_secs(30)))
        .map(|read| json!({"read":read}))
    }
    Call::Prompt { target, text } | Call::PromptWait { target, text, .. }
      if target.starts_with("hc:") =>
    {
      identity::verify(paths, env, hc_target(target))
        .and_then(|()| client.send(hc_target(target), text))
    }
    Call::SendKeys { target, keys } if target.starts_with("hc:") => {
      let mut args = vec!["keys", hc_target(target), "--"];
      args.extend(keys.iter().map(String::as_str));
      identity::verify(paths, env, hc_target(target))
        .and_then(|()| client.call(&args, Duration::from_secs(30)))
    }
    Call::Wait {
      target, timeout_ms, ..
    } if target.starts_with("hc:") => wait(paths, env, &client, hc_target(target), *timeout_ms),
    Call::AgentList
    | Call::Read { .. }
    | Call::Prompt { .. }
    | Call::PromptWait { .. }
    | Call::SendKeys { .. }
    | Call::Wait { .. }
    | Call::Start { .. }
    | Call::Create { .. }
    | Call::Remove { .. }
    | Call::WorkspaceList
    | Call::WorktreeList { .. }
    | Call::WorktreeOpen { .. }
    | Call::PaneList { .. }
    | Call::PaneInfo { .. }
    | Call::Notify { .. } => return None,
  };
  Some(Ok(render(result)))
}

fn render(result: Result<Value, toolu_mux::hc::Error>) -> String {
  match result {
    Ok(value) => value.to_string(),
    Err(error) => json!({"error":{"code": if error.code == "not_found" { "agent_not_found" } else { &error.code },
      "message":error.message,"uncertain":error.uncertain}}).to_string(),
  }
}

fn wait(
  paths: &Paths,
  env: &Env,
  client: &Client<'_>,
  target: &str,
  timeout_ms: u64,
) -> Result<Value, toolu_mux::hc::Error> {
  let seconds = Duration::from_millis(timeout_ms).as_secs_f64().to_string();
  identity::verify(paths, env, target).and_then(|()| {
    client.call(
      &["wait", target, "--timeout", &seconds],
      Duration::from_millis(timeout_ms).saturating_add(Duration::from_secs(30)),
    )
  })
}

fn hc_target(target: &str) -> &str {
  target.strip_prefix("hc:").unwrap_or_default()
}

fn hydrate(paths: &Paths, env: &Env, client: &Client<'_>) -> Result<Value, toolu_mux::hc::Error> {
  let listed = client.call(&["ls", "--all"], Duration::from_secs(30))?;
  let rows = listed
    .get("terminals")
    .and_then(Value::as_array)
    .ok_or_else(|| toolu_mux::hc::Error::new("invalid_output", "hc omitted terminals"))?;
  let mut terminals = Vec::new();
  for row in rows {
    let address = toolu_mux::hc::address(row)?;
    terminals.push(client.get(address)?);
  }
  for terminal in identity::tracked(paths, env, client)? {
    let address = toolu_mux::hc::address(&terminal)?;
    if !terminals
      .iter()
      .any(|item| toolu_mux::hc::address(item).ok() == Some(address))
    {
      terminals.push(terminal);
    }
  }
  Ok(json!({"terminals": terminals}))
}

/// Normalize hc facts into the engine's agent snapshot contract.
pub(crate) fn snapshot(listed: &Value) -> Value {
  let agents: Vec<Value> = listed
    .get("terminals")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter(|terminal| {
      terminal
        .pointer("/labels/toolu.owner")
        .and_then(Value::as_str)
        .is_some()
    })
    .filter_map(agent)
    .collect();
  json!({"agents":agents,"backend":"herdr-app"})
}

fn agent(terminal: &Value) -> Option<Value> {
  let address = terminal.get("address")?.as_str()?;
  let target = format!("hc:{address}");
  Some(
    json!({"name":target,"pane_id":target,"cwd":terminal.get("cwd"),
    "agent_status":status(terminal),"agent":terminal.pointer("/status/app"),
    "terminal_title_stripped":terminal.get("title")}),
  )
}

#[cfg(test)]
#[path = "tests/hc_backend_test.rs"]
mod tests;

/// Restore the immutable worker target before subscribing or sending input.
pub(crate) fn overlay_target(issue: &mut crate::model::Issue, record: &Value) {
  if let Some(agent) = record.get("agent").and_then(Value::as_str) {
    agent.clone_into(&mut issue.agent);
  }
  if let Some(pane) = record.get("pane_id").and_then(Value::as_str) {
    issue.pane = Some(pane.to_owned());
  }
  if let Some(worktree) = record.get("worktree").and_then(Value::as_str) {
    issue.worktree = Some(worktree.to_owned());
  }
}
