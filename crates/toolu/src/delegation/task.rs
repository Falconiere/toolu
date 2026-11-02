//! Caller-side snapshots and verified result import.

use super::{ArgMatches, Env, Submission, Value, client, json, value};
use std::time::Duration;
use toolu_mux::hc::remote;
use toolu_runtime::process::{Spec, run as process};

/// Execute one standalone delegation operation.
pub(super) fn run(matches: &ArgMatches, env: &Env) -> Result<Value, String> {
  let (verb, args) = matches
    .subcommand()
    .ok_or("a delegate operation is required")?;
  match verb {
    "submit" => submit(args, env),
    "status" => {
      serde_json::to_value(client::get(env, value(args, "key"))?).map_err(|e| e.to_string())
    }
    "cancel" => {
      serde_json::to_value(client::cancel(env, value(args, "key"))?).map_err(|e| e.to_string())
    }
    "retry" => {
      let key = value(args, "key");
      let outbox =
        toolu_mux::relay::store::Store::new(&toolu_mux::relay::config::root(env).join("outbox"));
      let request = outbox.load(key)?.request;
      serde_json::to_value(client::submit(env, &request, &outbox.input(key)?)?)
        .map_err(|e| e.to_string())
    }
    "results" => results(args, env),
    "report" => report(args, env),
    _ => Err("unknown delegate operation".into()),
  }
}

fn report(args: &ArgMatches, env: &Env) -> Result<Value, String> {
  let cwd = toolu_runtime::invocation::current_dir().map_err(|e| e.to_string())?;
  let repo = args.get_one::<String>("repo").map_or(
    cwd.to_str().ok_or("non-UTF-8 working directory")?,
    String::as_str,
  );
  let base = value(args, "base");
  let commit = value(args, "commit");
  if git(env, repo, &["rev-parse", "HEAD^{commit}"])? != commit || base.len() != commit.len() {
    return Err("report commit differs from HEAD".into());
  }
  if value(args, "status") == "done" {
    if !git(env, repo, &["status", "--porcelain"])?.is_empty() {
      return Err("commit all work before reporting success".into());
    }
    git(env, repo, &["merge-base", "--is-ancestor", base, commit])?;
  }
  let document = json!({"attempt":value(args,"attempt"), "base":base, "commit":commit, "status":value(args,"status"), "note":value(args,"note")});
  let path = std::path::Path::new(value(args, "file"));
  if path.exists() {
    let saved: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
      .map_err(|e| e.to_string())?;
    if saved != document {
      return Err("worker report conflicts with an existing outcome".into());
    }
  } else if !toolu_state::io::write_atomic(path, &document.to_string()) {
    return Err("cannot save worker report".into());
  }
  std::fs::File::open(path.parent().ok_or("missing report parent")?)
    .and_then(|dir| dir.sync_all())
    .map_err(|e| e.to_string())?;
  Ok(document)
}

fn submit(args: &ArgMatches, env: &Env) -> Result<Value, String> {
  let key = value(args, "key");
  let repo = value(args, "repo");
  let request = {
    if !git(env, repo, &["status", "--porcelain"])?.is_empty() {
      return Err("commit intended inputs before delegating".into());
    }
    let agent = value(args, "agent");
    let mut command = vec![if agent == "cursor" {
      "cursor-agent".into()
    } else {
      agent.into()
    }];
    if agent == "codex" {
      command.push("--no-daemon".into());
    }
    command.extend(
      args
        .get_many::<String>("args")
        .into_iter()
        .flatten()
        .cloned(),
    );
    Submission {
      key: key.into(),
      origin: git(env, repo, &["remote", "get-url", "origin"])?,
      base: git(env, repo, &["rev-parse", "HEAD^{commit}"])?,
      brief: std::fs::read_to_string(value(args, "brief")).map_err(|e| e.to_string())?,
      agent: agent.into(),
      argv: command,
      machine: args.get_one::<String>("machine").cloned(),
      group: args.get_one::<String>("group").cloned(),
    }
  };
  let bundle = remote::export_recover(env, repo, key, &request.base).map_err(|e| e.message)?;
  serde_json::to_value(client::submit(env, &request, &bundle)?).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "tests/task_test.rs"]
mod tests;

fn results(args: &ArgMatches, env: &Env) -> Result<Value, String> {
  let key = value(args, "key");
  let task = client::get(env, key)?;
  let commit = task
    .result
    .as_deref()
    .ok_or("task has no verified result")?;
  let attempt = task.attempt.as_deref().ok_or("task has no attempt")?;
  let repo = value(args, "repo");
  if git(env, repo, &["remote", "get-url", "origin"])? != task.request.origin {
    return Err("repository origin differs".into());
  }
  let bundle = client::output(env, key)?;
  remote::import_recover(env, repo, attempt, commit, &bundle).map_err(|e| e.message)?;
  Ok(json!({"key":key, "commit":commit, "ref":format!("refs/toolu-transfer/{attempt}/snapshot")}))
}

fn git(env: &Env, repo: &str, args: &[&str]) -> Result<String, String> {
  let mut spec = Spec::new(["git", "-C", repo]);
  spec.argv.extend(args.iter().map(|s| (*s).into()));
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(120);
  let out = process(&spec).map_err(|e| format!("{e:?}"))?;
  if out.exit_code != 0 || out.timed_out || out.truncated {
    return Err(format!("git failed: {}", out.stderr));
  }
  Ok(out.stdout.trim_end().into())
}
