//! Native hc launch with durable ownership and immutable terminal addresses.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_mux::Backend;
use toolu_mux::hc::{Client, Launch, address, require_owner, status};
use toolu_runtime::env::Env;

use super::Ctx;
use super::attempt::Attempt;
use super::brief::{BriefHost, BriefPaths, render_brief};
use super::failure::{Failure, argv, run_checked};
use super::record::{Record, text};
use crate::hosts::{AgentOpts, HostKind, agent_args};

/// Preserve recorded backend choices across controller restarts.
pub(super) fn backend(record: &Record, env: &Env) -> Result<Backend, Failure> {
  match text(record, "backend") {
    Some("herdr-app") => Ok(Backend::HerdrApp),
    Some("herdr") => Ok(Backend::Herdr),
    Some("tmux") => Ok(Backend::Tmux),
    Some(_) => Err(Failure::refusal("unknown recorded worker backend")),
    None if !record.is_empty() => Ok(
      if text(record, "pane_id").is_some_and(|p| p.starts_with('%')) {
        Backend::Tmux
      } else {
        Backend::Herdr
      },
    ),
    None => toolu_mux::backend::configured(env).map_err(Failure::refusal),
  }
}

/// Pin the launch client before reservation or lifecycle calls.
pub(super) fn pin(record: &mut Record, ctx: &mut Ctx<'_>, state: &Path) -> Result<(), Failure> {
  if record.is_empty()
    && crate::hc_target::load(state)
      .map_err(Failure::refusal)?
      .is_object()
  {
    record.insert("backend".into(), json!("herdr-app"));
  }
  let backend = backend(record, ctx.env)?;
  ctx.herdr.client.pin(backend);
  if record.is_empty() {
    record.insert("backend".into(), json!(backend.name()));
  }
  Ok(())
}

/// Preserve structured hc failures at the launch boundary.
pub(super) fn failed(error: toolu_mux::hc::Error) -> Failure {
  Failure {
    message: format!("hc {}: {}", error.code, error.message),
    exit_code: None,
    code: Some(error.code),
    uncertain: error.uncertain,
  }
}

pub(super) fn git(env: &Env, cwd: &str, args: &[&str]) -> Result<String, Failure> {
  let mut command = argv(&["git", "-C", cwd]);
  command.extend(args.iter().map(|arg| (*arg).into()));
  run_checked(&command, None, env, Duration::from_secs(120)).map(|s| s.trim_end().into())
}

pub(super) fn local_worktree(
  env: &Env,
  checkout: &str,
  branch: &str,
  base: &str,
  path: &str,
) -> Result<(), Failure> {
  let listed = git(env, checkout, &["worktree", "list", "--porcelain", "-z"])?;
  let expected = format!("branch refs/heads/{branch}");
  let existing = listed
    .split("\0\0")
    .find(|block| block.split('\0').any(|line| line == expected));
  if let Some(block) = existing {
    let listed_path = block
      .split('\0')
      .find_map(|line| line.strip_prefix("worktree "))
      .ok_or_else(|| Failure::refusal("worktree path missing"))?;
    let real = |p| {
      std::fs::canonicalize(p)
        .map_err(|e| Failure::refusal(format!("cannot resolve worktree: {e}")))
    };
    if real(listed_path)? != real(path)? {
      return Err(Failure::refusal("worktree path identity changed"));
    }
    return Ok(());
  }
  git(
    env,
    checkout,
    &["worktree", "add", "-b", branch, path, base],
  )
  .map(|_| ())
}

fn host_argv(run: &Attempt<'_>) -> Result<Vec<String>, Failure> {
  let binary = if run.host.kind == HostKind::Cursor {
    "cursor-agent"
  } else {
    run.host.kind.name()
  };
  let mut args = vec![binary.to_owned()];
  args.extend(
    agent_args(
      run.host.kind,
      &AgentOpts {
        key: run.issue.key.clone(),
        model: run.host.model.clone(),
        effort: run.host.effort.clone(),
        bypass: !run.opts.safe,
        permission_mode: run.opts.permission_mode.clone(),
        ..AgentOpts::default()
      },
    )
    .map_err(Failure::refusal)?,
  );
  Ok(args)
}

impl Attempt<'_> {
  pub(super) fn hc_go(&mut self, record: &mut Record, ctx: &mut Ctx<'_>) -> Result<Value, Failure> {
    let command = host_argv(self)?;
    if self.opts.dry_run {
      return Ok(
        json!({"dry_run":true,"issue":self.issue.reference,"backend":"herdr-app",
        "commands":["git worktree add <worktree> <committed-base>", "hc run --bg --no-parent --outlive-parent -- <host>"],"agent_args":command}),
      );
    }
    Client::new(ctx.env)
      .call(&["ls"], Duration::from_secs(10))
      .map_err(failed)?;
    self.hc_recover(record, ctx)?;
    if record.get("hc_prepared") != Some(&Value::Bool(true)) {
      self.hc_prepare(record, ctx)?;
    }
    let terminal = self.hc_start(record, ctx, &command)?;
    let paths = BriefPaths::of(
      self.state(),
      &self.issue.key,
      text(record, "worktree").unwrap_or(""),
    );
    self.hc_deliver(record, ctx, &terminal, &paths)?;
    self.persist(record, "running")?;
    Ok(super::live_result(&self.issue.reference, &self.log, record))
  }

  fn hc_start(
    &mut self,
    record: &mut Record,
    ctx: &Ctx<'_>,
    command: &[String],
  ) -> Result<Value, Failure> {
    let client = Client::new(ctx.env);
    let name = text(record, "terminal_name").unwrap_or("");
    let owner = text(record, "terminal_owner").unwrap_or("");
    let saved = text(record, "terminal_address");
    let machine = text(record, "machine").filter(|_| remote_worker(record));
    let target = machine.map_or_else(|| name.to_owned(), |m| format!("{m}/{name}"));
    let cwd = worker_cwd(record);
    let terminal = match client.get(saved.unwrap_or(&target)) {
      Ok(terminal) => terminal,
      Err(error) if error.code == "not_found" && saved.is_none() => client
        .launch(&Launch {
          name,
          owner,
          cwd,
          argv: command,
          machine,
        })
        .map_err(failed)?,
      Err(error) => return Err(failed(error)),
    };
    require_owner(&terminal, owner).map_err(failed)?;
    if terminal.get("cwd").and_then(Value::as_str) != Some(cwd) {
      return Err(Failure::refusal("hc terminal worktree identity differs"));
    }
    if terminal.get("state").and_then(Value::as_str) == Some("finished") {
      return Err(Failure::refusal(
        "recorded hc terminal has finished; reconcile before replacement",
      ));
    }
    let address = address(&terminal).map_err(failed)?.to_owned();
    record.insert("terminal_address".into(), json!(address));
    record.insert("pane_id".into(), json!(format!("hc:{address}")));
    record.insert("agent".into(), json!(format!("hc:{address}")));
    record.insert("workspace_id".into(), json!(format!("hc:{address}")));
    self.persist(record, "starting")?;
    Ok(terminal)
  }

  fn hc_deliver(
    &self,
    record: &mut Record,
    ctx: &Ctx<'_>,
    terminal: &Value,
    paths: &BriefPaths,
  ) -> Result<(), Failure> {
    if text(record, "prompt_state") == Some("acknowledged") {
      return Ok(());
    }
    let client = Client::new(ctx.env);
    let address = address(terminal).map_err(failed)?;
    if text(record, "prompt_state") == Some("submitting") {
      let _observed = client.call(&["read", address], Duration::from_secs(30));
      return Err(Failure::refusal(
        "previous hc send is uncertain; inspect delivery before retrying",
      ));
    }
    let terminal = ready(&client, address, ctx.session_wait)?;
    if status(&terminal) == "blocked" {
      record.insert("lifecycle_outcome".into(), json!("blocked"));
      return Err(Failure {
        code: Some(super::failure::APPROVAL.into()),
        ..Failure::refusal("hc worker is blocked; show it to answer explicitly")
      });
    }
    let base = text(record, "base").unwrap_or("main").to_owned();
    let brief = render_brief(
      self.graph,
      self.issue,
      paths,
      &base,
      BriefHost {
        kind: self.host.kind,
        safe: self.opts.safe,
      },
    )
    .map_err(Failure::unknown)?;
    std::fs::create_dir_all(Path::new(&paths.brief).parent().unwrap_or(self.state()))
      .map_err(|e| Failure::unknown(e.to_string()))?;
    let prompt = self.hc_brief(record, ctx, paths, &brief)?;
    record.insert("brief".into(), json!(paths.brief));
    record.insert("status_file".into(), json!(paths.status));
    record.insert("prompt_state".into(), json!("submitting"));
    self.persist(record, "starting")?;
    client.send(address, &prompt).map_err(failed)?;
    record.insert("prompt_state".into(), json!("acknowledged"));
    record.insert("launches".into(), json!(self.launches.max(1)));
    record.insert("lifecycle_outcome".into(), json!("started"));
    Ok(())
  }
}

/// Retained remote cleanup metadata does not move a local delivery worker.
pub(super) fn remote_worker(record: &Record) -> bool {
  text(record, "machine").is_some() && record.get("controller_review") != Some(&Value::Bool(true))
}

fn worker_cwd(record: &Record) -> &str {
  if remote_worker(record) {
    text(record, "remote_worktree").unwrap_or("")
  } else {
    text(record, "worktree").unwrap_or("")
  }
}

fn ready(client: &Client<'_>, address: &str, wait: Duration) -> Result<Value, Failure> {
  let deadline = Instant::now() + wait;
  loop {
    let terminal = client.get(address).map_err(failed)?;
    match status(&terminal) {
      "idle" | "done" | "blocked"
        if terminal.get("state").and_then(Value::as_str) != Some("finished") =>
      {
        return Ok(terminal);
      }
      _ if Instant::now() >= deadline => {
        return Err(Failure::unknown("hc worker did not report readiness"));
      }
      _ => std::thread::sleep(Duration::from_millis(100)),
    }
  }
}

#[cfg(test)]
#[path = "tests/hc_test.rs"]
mod tests;
