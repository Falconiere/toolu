//! Preparation and result verification on a configured worker.

use super::durable;
use crate::hc::remote::{Target, Transfer};
use crate::relay::{
  config::Machine,
  store::{Store, Task},
};
use std::path::Path;
use toolu_runtime::env::Env;

mod bootstrap;
mod clone;

fn error(error: &crate::hc::Error) -> String {
  format!("{}: {}", error.code, error.message)
}

/// Prepare an isolated checkout and return its reporting executable.
pub(super) fn prepare(
  root: &Path,
  env: &Env,
  store: &Store,
  task: &Task,
) -> Result<String, String> {
  let machine = task.worker.as_ref().ok_or("missing worker")?;
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let repo = repository(env, task, machine)?;
  let worktree = format!("{}/tasks/{attempt}", machine.root.trim_end_matches('/'));
  let transfer = Transfer {
    target: Target {
      machine: &machine.name,
      repo: &repo,
    },
    env,
    run: &task.request.key,
  };
  let binary_transfer = Transfer {
    target: transfer.target.clone(),
    env,
    run: attempt,
  };
  let binary = bootstrap::reporting_binary(&binary_transfer, env)?;
  import_input(&transfer, store, task, &binary)?;
  checkout(&transfer, task, &worktree)?;
  let checkout = Transfer {
    target: Target {
      machine: &machine.name,
      repo: &worktree,
    },
    env,
    run: attempt,
  };
  build_prerequisites(&checkout)?;
  recipes(root, attempt, &checkout, Some(&task.recipes))?;
  Ok(binary)
}

fn import_input(
  transfer: &Transfer<'_>,
  store: &Store,
  task: &Task,
  binary: &str,
) -> Result<(), String> {
  transfer
    .preflight_with(
      &task.request.origin,
      &format!("toolu {}", env!("CARGO_PKG_VERSION")),
      &task.request.agent,
      binary,
    )
    .map_err(|e| error(&e))?;
  transfer
    .upload(
      "input",
      &task.request.base,
      &store.input(&task.request.key)?,
    )
    .map_err(|e| error(&e))
}

fn build_prerequisites(checkout: &Transfer<'_>) -> Result<(), String> {
  let files = checkout.exec(&["ls", "-A"], &[]).map_err(|e| error(&e))?;
  if String::from_utf8_lossy(&files)
    .lines()
    .any(|name| name == "Cargo.toml")
  {
    checkout
      .exec(&["rustc", "--version"], &[])
      .map_err(|e| error(&e))?;
    checkout
      .exec(&["cargo", "--version"], &[])
      .map_err(|e| error(&e))?;
  }
  Ok(())
}

fn checkout(transfer: &Transfer<'_>, task: &Task, worktree: &str) -> Result<(), String> {
  let paths = transfer
    .git(&["worktree", "list", "--porcelain"])
    .map_err(|e| error(&e))?;
  if !paths
    .lines()
    .any(|line| line == format!("worktree {worktree}"))
  {
    transfer
      .git(&["worktree", "add", "--detach", worktree, &task.request.base])
      .map_err(|e| error(&e))?;
  }
  let checkout = Transfer {
    target: Target {
      machine: transfer.target.machine,
      repo: worktree,
    },
    env: transfer.env,
    run: transfer.run,
  };
  if checkout
    .git(&["rev-parse", "HEAD"])
    .map_err(|e| error(&e))?
    != task.request.base
    || !checkout
      .git(&["status", "--porcelain"])
      .map_err(|e| error(&e))?
      .is_empty()
  {
    return Err("prepared worktree differs from the input commit".into());
  }
  Ok(())
}

fn repository(env: &Env, task: &Task, machine: &Machine) -> Result<String, String> {
  let bootstrap = Transfer {
    target: Target {
      machine: &machine.name,
      repo: "/",
    },
    env,
    run: &task.request.key,
  };
  let hash = String::from_utf8(
    bootstrap
      .exec(
        &["git", "hash-object", "--stdin"],
        task.request.origin.as_bytes(),
      )
      .map_err(|e| error(&e))?,
  )
  .map_err(|e| e.to_string())?
  .trim()
  .to_owned();
  if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
    return Err("invalid origin hash".into());
  }
  let repo = format!("{}/repos/{hash}", machine.root.trim_end_matches('/'));
  clone::ensure(&bootstrap, task, machine, &hash, &repo)?;
  Ok(repo)
}

fn recipes(
  root: &Path,
  attempt: &str,
  checkout: &Transfer<'_>,
  recipe: Option<&Vec<Vec<String>>>,
) -> Result<(), String> {
  let dir = root.join("setup").join(attempt);
  for (index, argv) in recipe.into_iter().flatten().enumerate() {
    durable::ensure_dir(&dir).map_err(|e| format!("uncertain recipe: {e}"))?;
    let intent = dir.join(format!("{index}.intent"));
    let done = dir.join(format!("{index}.done"));
    let command = serde_json::to_vec(argv).map_err(|e| e.to_string())?;
    if completed(&intent, &done, &command)? {
      continue;
    }
    durable::create(&intent, &command).map_err(|e| format!("uncertain recipe: {e}"))?;
    let args: Vec<_> = argv.iter().map(String::as_str).collect();
    checkout
      .exec(&args, &[])
      .map_err(|e| format!("uncertain recipe: {}", error(&e)))?;
    durable::create(&done, b"done").map_err(|e| format!("uncertain recipe: {e}"))?;
  }
  Ok(())
}

fn completed(intent: &Path, done: &Path, command: &[u8]) -> Result<bool, String> {
  if !intent.exists() {
    return Ok(false);
  }
  if std::fs::read(intent).map_err(|e| e.to_string())? != command || !done.exists() {
    return Err("uncertain recipe: prior setup may have run".into());
  }
  Ok(true)
}

/// Treat unreadable setup state as possible side effects.
pub(super) fn has_recipe_intent(root: &Path, attempt: &str) -> bool {
  match std::fs::read_dir(root.join("setup").join(attempt)) {
    Ok(entries) => entries.into_iter().any(|entry| {
      entry.map_or(true, |entry| {
        entry.file_name().to_string_lossy().ends_with(".intent")
      })
    }),
    Err(error) => error.kind() != std::io::ErrorKind::NotFound,
  }
}

/// Verify and export a completed committed result.
pub(super) fn result(
  env: &Env,
  task: &Task,
  machine: &Machine,
  cwd: &str,
) -> Result<(String, Vec<u8>), String> {
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let transfer = Transfer {
    target: Target {
      machine: &machine.name,
      repo: cwd,
    },
    env,
    run: attempt,
  };
  if !transfer
    .git(&["status", "--porcelain"])
    .map_err(|e| error(&e))?
    .is_empty()
  {
    return Err("worker left uncommitted changes".into());
  }
  let head = transfer
    .git(&["rev-parse", "HEAD"])
    .map_err(|e| error(&e))?;
  transfer
    .git(&["merge-base", "--is-ancestor", &task.request.base, &head])
    .map_err(|e| error(&e))?;
  let bytes = transfer
    .export_recover("output", &head)
    .map_err(|e| error(&e))?;
  Ok((head, bytes))
}

/// Read a report bound to this attempt and input commit.
pub(super) fn reported(
  env: &Env,
  task: &Task,
  machine: &Machine,
  cwd: &str,
) -> Option<serde_json::Value> {
  let attempt = task.attempt.as_deref()?;
  let transfer = Transfer {
    target: Target {
      machine: &machine.name,
      repo: cwd,
    },
    env,
    run: attempt,
  };
  let path = format!(
    "{}/tasks/{attempt}-report.json",
    machine.root.trim_end_matches('/')
  );
  let bytes = transfer.exec(&["cat", "--", &path], &[]).ok()?;
  let report: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
  matches_report(&report, attempt, &task.request.base).then_some(report)
}

fn matches_report(report: &serde_json::Value, attempt: &str, base: &str) -> bool {
  report.get("attempt").and_then(serde_json::Value::as_str) == Some(attempt)
    && report.get("base").and_then(serde_json::Value::as_str) == Some(base)
}

#[cfg(test)]
#[path = "tests/worker_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/worker_preparation_test.rs"]
mod preparation_tests;
