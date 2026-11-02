//! Enroll only a terminal proven to own the running host process.
use super::types::Wake;
use super::{
  client::directory,
  client_model::{save, state},
};
use serde_json::Value;
use std::{path::Path, time::Duration};
use toolu_runtime::{
  env::Env,
  process::{Spec, run},
};
use toolu_state::lock::{LockOptions, with_lock_strict};

fn command(env: &Env, args: &[&str]) -> Option<String> {
  let mut spec = Spec::new(args.iter().copied());
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_millis(200);
  spec.max_output_bytes = 128 * 1024;
  let out = run(&spec).ok()?;
  (out.exit_code == 0 && !out.timed_out && !out.truncated).then_some(out.stdout)
}
fn owns_host(env: &Env, info: &Value, host: &str) -> bool {
  let Some(pid) = info
    .pointer("/foreground/pid")
    .and_then(Value::as_u64)
    .filter(|pid| *pid > 0)
  else {
    return false;
  };
  let executable = info
    .pointer("/foreground/argv/0")
    .and_then(Value::as_str)
    .unwrap_or("");
  if Path::new(executable)
    .file_name()
    .and_then(|name| name.to_str())
    != Some(host)
  {
    return false;
  }
  let Some(table) = command(env, &["ps", "-axo", "pid=,ppid="]) else {
    return false;
  };
  let mut current = u64::from(std::process::id());
  for _ in 0..32 {
    if current == pid {
      return true;
    }
    let parent = table.lines().find_map(|line| {
      let mut fields = line.split_whitespace();
      let child = fields.next()?.parse::<u64>().ok()?;
      let parent = fields.next()?.parse::<u64>().ok()?;
      (child == current).then_some(parent)
    });
    let Some(parent) = parent else {
      return false;
    };
    if parent == current {
      return false;
    }
    current = parent;
  }
  false
}
/// Register wake metadata only when the current host owns the terminal.
/// # Errors
/// Local state cannot be read or persisted. Unverified terminals are skipped.
pub fn enroll(env: &Env, id: &str, machine: &str, host: &str) -> Result<(), String> {
  let Some(terminal) = env.get("HC_TERMINAL_ID") else {
    return Ok(());
  };
  let Some(number) = terminal.strip_prefix('t') else {
    return Ok(());
  };
  if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(());
  }
  let Some(info) =
    command(env, &["hc", "get", terminal]).and_then(|s| serde_json::from_str::<Value>(&s).ok())
  else {
    return Ok(());
  };
  if info.get("address").and_then(Value::as_str) != Some(terminal) || !owns_host(env, &info, host) {
    return Ok(());
  }
  let Some(created) = info
    .get("created_ms")
    .and_then(Value::as_u64)
    .filter(|n| *n > 0)
  else {
    return Ok(());
  };
  let path = directory(env, id)?.join("state.json");
  let options = LockOptions {
    stale: Duration::MAX,
    ..LockOptions::default()
  };
  with_lock_strict(&path, options, || {
    let mut state = state(&path)?;
    let generation = &state.session.generation;
    let owner = format!("{id}:{generation}");
    let label = format!("toolu.peers={owner}");
    if command(env, &["hc", "label", terminal, &label]).is_none() {
      return Ok(());
    }
    state.session.wake = Some(Wake {
      address: format!("{machine}/{terminal}"),
      owner,
      created_ms: created,
      pid: info
        .pointer("/foreground/pid")
        .and_then(Value::as_u64)
        .unwrap_or(0),
    });
    save(&path, &state)
  })?
}

#[cfg(test)]
#[path = "tests/client_wake_test.rs"]
mod tests;
