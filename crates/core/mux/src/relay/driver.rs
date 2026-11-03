//! Serialized dispatch and recovery for relay tasks.

use super::{
  config,
  store::{Store, Task},
  worker,
};
use crate::hc::{self, Client, Launch};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, with_lock_strict};

/// Perform one dispatch and recovery iteration.
/// # Errors
/// The dispatch lock, ledger, or configuration cannot be read or written.
pub fn tick(root: &Path, env: &Env) -> Result<Vec<Task>, String> {
  super::durable::ensure_dir(root)?;
  let options = LockOptions {
    stale: Duration::MAX,
    ..LockOptions::default()
  };
  with_lock_strict(&root.join("dispatch"), options, || run(root, env))?
}

fn run(root: &Path, env: &Env) -> Result<Vec<Task>, String> {
  let store = Store::new(root);
  let config = config::load(root)?;
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|e| e.to_string())?
    .as_secs();
  for item in store.list()? {
    let task = if item.phase == "queued" {
      store.reserve(&item.request.key, &config, now)?
    } else {
      Some(item)
    };
    if let Some(task) = task
      && matches!(
        task.phase.as_str(),
        "preparing" | "running" | "held" | "cancelling"
      )
      && let Err(error) = process((root, env), &store, &config, &task, now)
    {
      let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
      store.annotate(&task.request.key, attempt, &error)?;
    }
  }
  store.list()
}

fn process(
  context: (&Path, &Env),
  store: &Store,
  config: &config::Config,
  task: &Task,
  now: u64,
) -> Result<(), String> {
  let (root, env) = context;
  let machine = task
    .worker
    .as_ref()
    .ok_or("missing reserved worker settings")?;
  let mut current = config
    .machines
    .get(&machine.name)
    .cloned()
    .ok_or("reserved worker missing")?;
  current.draining = machine.draining;
  if current != *machine {
    return Err("reserved worker settings changed".into());
  }
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let name = terminal_name(attempt);
  let cwd = format!("{}/tasks/{attempt}", machine.root.trim_end_matches('/'));
  let owner = format!("toolu.owner={attempt}");
  if task.phase == "preparing" {
    match worker::prepare(root, env, store, task) {
      Ok(binary) => {
        let prepared = store.prepared(&task.request.key, attempt, &binary)?;
        launch(store, env, &prepared, (&name, &cwd, &owner))?;
      }
      Err(error) => {
        if error.starts_with("uncertain recipe:") || worker::has_recipe_intent(root, attempt) {
          store.launched(&task.request.key, attempt, None, &error)?;
        } else {
          store.retry(&task.request.key, attempt, now, &error)?;
        }
      }
    }
  } else {
    recover((root, env), store, task, (&name, &cwd, &owner))?;
  }
  Ok(())
}

fn launch(
  store: &Store,
  env: &Env,
  task: &Task,
  identity: (&str, &str, &str),
) -> Result<(), String> {
  let (name, cwd, owner) = identity;
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let binary = task
    .worker_binary
    .as_deref()
    .ok_or("missing reporting executable")?;
  let report_path = format!(
    "{}/tasks/{attempt}-report.json",
    task
      .worker
      .as_ref()
      .ok_or("missing worker")?
      .root
      .trim_end_matches('/')
  );
  store.launched(&task.request.key, attempt, None, "launch intent")?;
  let mut argv = task.request.argv.clone();
  argv.push(format!(
    "{}\n\nCommit your results. Do not push, review, or merge. Report completion with: {} delegate report --file {} --attempt {} --base {} --status done --commit <HEAD>. On failure report --status failed with --note. Then exit.",
    task.request.brief, quote(binary), quote(&report_path), attempt, task.request.base
  ));
  let client = Client::new(env);
  let launch = Launch {
    name,
    cwd,
    owner,
    machine: task.machine.as_deref(),
    argv: &argv,
  };
  if let Ok(terminal) = client.launch(&launch)
    && terminal.get("cwd").and_then(serde_json::Value::as_str) == Some(cwd)
  {
    let address = hc::address(&terminal).map_err(|e| e.message)?;
    store.launched(&task.request.key, attempt, Some(address), "launched")?;
  }
  Ok(())
}

fn recover(
  context: (&Path, &Env),
  store: &Store,
  task: &Task,
  identity: (&str, &str, &str),
) -> Result<(), String> {
  let (root, env) = context;
  let (name, cwd, owner) = identity;
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let machine = task.worker.as_ref().ok_or("missing worker")?;
  let fallback = format!("{}/{name}", machine.name);
  let target = task.terminal.as_deref().unwrap_or(&fallback);
  let client = Client::new(env);
  let Some(terminal) = find_terminal((root, env), store, task, &client, target)? else {
    return Ok(());
  };
  hc::require_owner(&terminal, owner).map_err(|e| e.message)?;
  if terminal.get("cwd").and_then(serde_json::Value::as_str) != Some(cwd) {
    return Err("terminal working directory differs".into());
  }
  let finished = terminal.get("state").and_then(serde_json::Value::as_str) == Some("finished");
  if task.phase == "cancelling" {
    if !finished {
      client.kill_owned(target, owner).map_err(|e| e.message)?;
    }
    let settled = store.finish_cancel(&task.request.key, attempt)?;
    return settle(store, env, &settled, cwd);
  }
  let address = hc::address(&terminal).map_err(|e| e.message)?;
  let address = if address.contains('/') {
    address.to_owned()
  } else {
    format!("{}/{address}", machine.name)
  };
  if task.terminal.is_none() {
    store.launched(&task.request.key, attempt, Some(&address), "recovered")?;
  }
  if let Some(report) = worker::reported(env, task, machine, cwd)? {
    if !finished {
      client.kill_owned(target, owner).map_err(|e| e.message)?;
    }
    let settled = accept_report(store, env, task, cwd, &report)?;
    return settle(store, env, &settled, cwd);
  }
  if finished {
    let settled = store.fail(&task.request.key, attempt, "worker exited without a report")?;
    return settle(store, env, &settled, cwd);
  }
  note_prompt(store, task, &terminal, &address)
}

/// Say in the task note while the worker waits at a permission or trust
/// prompt, and restore `launched` once it moves on, which also clears a
/// diagnostic a failed tick left behind. Nothing is typed into it.
fn note_prompt(
  store: &Store,
  task: &Task,
  terminal: &serde_json::Value,
  address: &str,
) -> Result<(), String> {
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let note = task.note.as_deref().unwrap_or_default();
  let wanted = if hc::status(terminal) == "blocked" {
    format!("blocked: the agent waits on a prompt in {address}")
  } else if matches!(note, "launched" | "recovered") {
    return Ok(());
  } else {
    "launched".to_owned()
  };
  if note != wanted {
    store.annotate(&task.request.key, attempt, &wanted)?;
  }
  Ok(())
}

/// Remove the settled attempt's worktree, keeping the receipt when that fails.
fn settle(store: &Store, env: &Env, task: &Task, cwd: &str) -> Result<(), String> {
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let machine = task.worker.as_ref().ok_or("missing worker")?;
  if let Err(error) = worker::cleanup(env, machine, attempt, cwd) {
    let note = task.note.as_deref().unwrap_or_default();
    let note = if note.is_empty() {
      format!("cleanup incomplete: {error}")
    } else {
      format!("{note}; cleanup incomplete: {error}")
    };
    store.annotate(&task.request.key, attempt, &note)?;
  }
  Ok(())
}

fn find_terminal(
  context: (&Path, &Env),
  store: &Store,
  task: &Task,
  client: &Client<'_>,
  target: &str,
) -> Result<Option<serde_json::Value>, String> {
  let (root, env) = context;
  match client.get(target) {
    Ok(terminal) => Ok(Some(terminal)),
    Err(error)
      if error.code == "not_found" && task.phase == "cancelling" && !task.launch_started =>
    {
      let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
      if worker::has_recipe_intent(root, attempt) {
        return Err("terminal absent after recipe intent".into());
      }
      let settled = store.finish_cancel(&task.request.key, attempt)?;
      let cwd = format!(
        "{}/tasks/{attempt}",
        task
          .worker
          .as_ref()
          .ok_or("missing worker")?
          .root
          .trim_end_matches('/')
      );
      settle(store, env, &settled, &cwd)?;
      Ok(None)
    }
    Err(error) => Err(format!("terminal lookup {}: {}", error.code, error.message)),
  }
}

fn accept_report(
  store: &Store,
  env: &Env,
  task: &Task,
  cwd: &str,
  report: &serde_json::Value,
) -> Result<Task, String> {
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  match report.get("status").and_then(serde_json::Value::as_str) {
    Some("failed") => store.fail(
      &task.request.key,
      attempt,
      report
        .get("note")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("worker failed"),
    ),
    Some("done") => {
      let machine = task.worker.as_ref().ok_or("missing worker")?;
      let (head, bytes) = worker::result(env, task, machine, cwd)?;
      if report.get("commit").and_then(serde_json::Value::as_str) != Some(head.as_str()) {
        return Err("reported commit differs from worktree HEAD".into());
      }
      store.report(&task.request.key, attempt, &head, &bytes)
    }
    _ => Err("worker report has no known status".into()),
  }
}

fn terminal_name(attempt: &str) -> String {
  format!(
    "td-{}",
    attempt
      .chars()
      .filter(|c| *c != '-')
      .take(26)
      .collect::<String>()
  )
}

fn quote(value: &str) -> String {
  format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
#[path = "tests/driver_test.rs"]
mod tests;
