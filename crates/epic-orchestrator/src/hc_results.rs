//! Accept remote reports only with their run, base and committed result verified.

use crate::launch::record::{Record, text, write_record};
use crate::server::Engine;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use toolu_mux::hc::remote::{self, Target, Transfer};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

mod mirror;
mod review;

/// Read and import a remote result while holding the issue launch lock.
pub(crate) fn execute(state: &Path, key: &str, env: &Env) -> Result<Option<Value>, String> {
  crate::launch::admission::with_launch_ownership(state, key, || {
    execute_locked(state, key, env).map_err(crate::launch::Failure::unknown)
  })
  .map_err(|err| err.message)
}

fn execute_locked(state: &Path, key: &str, env: &Env) -> Result<Option<Value>, String> {
  let path = state.join("issues").join(format!("{key}.json"));
  let mut record = crate::launch::record::load_launch_record(&path)?;
  if text(&record, "key") != Some(key)
    || text(&record, "backend") != Some("herdr-app")
    || text(&record, "stage") != Some("running")
  {
    return Err("remote result issue ownership or stage changed".into());
  }
  if text(&record, "hc_session")
    .is_some_and(|session| session != env.get("HC_SESSION").unwrap_or("default"))
  {
    return Err("recorded HC_SESSION differs; restore it before result import".into());
  }
  if text(&record, "machine").is_none() || text(&record, "remote_report").is_none() {
    return Ok(None);
  }
  let Some(report) = pull(&record, env)? else {
    return Ok(None);
  };
  verify(&record, &report)?;
  if record.get("remote_report_seen") == Some(&report) {
    return Ok(None);
  }
  if report.get("phase").and_then(Value::as_str) == Some("ready") {
    accept(&mut record, &report, env)?;
    write_record(&path, &record)?;
    review::handoff(&path, &mut record, env)?;
  }
  Ok(Some(report))
}

/// Commit a completed report and refresh the persisted local worker identity.
pub(crate) fn complete(
  engine: &mut Engine,
  key: &str,
  result: Result<Option<Value>, String>,
) -> Result<(), String> {
  let report = match result {
    Ok(Some(report)) => report,
    Ok(None) => return Ok(()),
    Err(err) => {
      crate::logic::push_attention(
        &mut engine.world,
        "needs-human",
        key,
        &format!("remote result: {err}"),
      );
      return engine.flush();
    }
  };
  let Some(issue) = engine.world.issues.get(key).cloned() else {
    return Ok(());
  };
  let result =
    crate::launch::admission::with_launch_ownership(Path::new(&issue.state_dir), key, || {
      complete_locked(engine, key, &issue, &report).map_err(crate::launch::Failure::unknown)
    });
  if let Err(err) = result {
    crate::logic::push_attention(
      &mut engine.world,
      "needs-human",
      key,
      &format!("remote result: {}", err.message),
    );
    engine.flush()?;
  }
  Ok(())
}

fn complete_locked(
  engine: &mut Engine,
  key: &str,
  issue: &crate::model::Issue,
  report: &Value,
) -> Result<(), String> {
  let path = crate::snapshot::issue_path(&issue.state_dir, key);
  let record = crate::launch::record::load_launch_record(&path)?;
  if text(&record, "stage") != Some("running") {
    return Ok(());
  }
  verify(&record, report)?;
  if let Some(current) = engine.world.issues.get_mut(key) {
    if let Some(agent) = text(&record, "agent") {
      agent.clone_into(&mut current.agent);
    }
    current.pane = text(&record, "pane_id").map(str::to_owned);
  }
  let phase = report
    .get("phase")
    .and_then(Value::as_str)
    .unwrap_or("needs-human");
  engine.report(&crate::model::Report {
    key: key.to_owned(),
    epic: issue.epic.clone(),
    state_dir: issue.state_dir.clone(),
    phase: if phase == "ready" {
      "execution".into()
    } else {
      phase.into()
    },
    pr: issue.pr,
    note: report
      .get("note")
      .and_then(Value::as_str)
      .unwrap_or("")
      .into(),
  })?;
  let mut current = crate::launch::record::load_launch_record(&path)?;
  if text(&current, "stage") == Some("running") {
    current.insert("remote_report_seen".into(), report.clone());
    write_record(&path, &current)?;
  }
  Ok(())
}

fn field<'a>(record: &'a Record, name: &str) -> Result<&'a str, String> {
  text(record, name)
    .filter(|s| !s.is_empty())
    .ok_or_else(|| format!("missing remote {name}"))
}

fn transfer<'a>(record: &'a Record, env: &'a Env, run: &'a str) -> Result<Transfer<'a>, String> {
  let target = Target::new(
    Some(field(record, "machine")?),
    Some(field(record, "remote_repo")?),
  )
  .map_err(|e| e.message)?
  .ok_or("missing remote target")?;
  Ok(Transfer { target, env, run })
}

fn pull(record: &Record, env: &Env) -> Result<Option<Value>, String> {
  let run = format!("{}-in", field(record, "hc_run")?);
  let remote = transfer(record, env, &run)?;
  let file = field(record, "remote_report")?;
  let parent = Path::new(file)
    .parent()
    .and_then(Path::to_str)
    .ok_or("invalid report path")?;
  let files = remote
    .exec(
      &[
        "find",
        parent,
        "-maxdepth",
        "1",
        "-type",
        "f",
        "-name",
        "content",
        "-print",
      ],
      &[],
    )
    .map_err(|e| e.message)?;
  if files.is_empty() {
    return Ok(None);
  }
  let bytes = remote
    .exec(&["cat", "--", file], &[])
    .map_err(|e| e.message)?;
  serde_json::from_slice(&bytes)
    .map(Some)
    .map_err(|_error| "invalid remote report JSON".into())
}

fn verify(record: &Record, report: &Value) -> Result<(), String> {
  let string = |key| {
    report
      .get(key)
      .and_then(Value::as_str)
      .ok_or("missing remote report identity")
  };
  let (run, base, commit) = (string("run")?, string("expected_base")?, string("commit")?);
  crate::hc_report::validate(run, base, commit)?;
  if run != field(record, "hc_run")? || base != field(record, "expected_base")? {
    return Err("remote report run or base differs from dispatch".into());
  }
  if !matches!(
    string("phase")?,
    "execution" | "ready" | "needs-human" | "failed"
  ) {
    return Err("unsupported remote worker phase".into());
  }
  Ok(())
}

fn accept(record: &mut Record, report: &Value, env: &Env) -> Result<(), String> {
  let commit = report
    .get("commit")
    .and_then(Value::as_str)
    .ok_or("missing result commit")?;
  if record.get("remote_result_imported") == Some(&Value::Bool(true)) {
    if text(record, "remote_result_commit") == Some(commit) {
      return Ok(());
    }
    return Err("remote result changed after acceptance; inspect before another transfer".into());
  }
  let run = format!("{}-out", field(record, "hc_run")?);
  let remote = transfer(record, env, &run)?;
  let tree = field(record, "remote_worktree")?;
  verify_remote(&remote, tree, field(record, "expected_base")?, commit)?;
  let bytes = remote
    .export_recover("result", commit)
    .map_err(|e| e.message)?;
  let checkout = field(record, "checkout")?;
  remote::import_recover(env, checkout, &run, commit, &bytes).map_err(|e| e.message)?;
  mirror::advance_mirror(record, env, commit)?;
  record.insert("remote_result_commit".into(), json!(commit));
  record.insert("remote_result_imported".into(), json!(true));
  Ok(())
}

fn verify_remote(
  remote: &Transfer<'_>,
  tree: &str,
  base: &str,
  commit: &str,
) -> Result<(), String> {
  if remote
    .git(&["-C", tree, "rev-parse", "HEAD"])
    .map_err(|e| e.message)?
    != commit
  {
    return Err("remote HEAD differs from reported commit".into());
  }
  if !remote
    .git(&["-C", tree, "status", "--porcelain", "--untracked-files=all"])
    .map_err(|e| e.message)?
    .is_empty()
  {
    return Err("remote worktree is dirty; committed result retained for recovery".into());
  }
  remote
    .git(&["merge-base", "--is-ancestor", base, commit])
    .map_err(|e| e.message)?;
  Ok(())
}

fn command(env: &Env, args: &[&str]) -> Result<String, String> {
  let mut spec = Spec::new(args.iter().copied());
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(120);
  let output = run(&spec).map_err(|e| format!("{e:?}"))?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Err(format!(
      "{} failed: {}",
      args.first().copied().unwrap_or("command"),
      output.stderr.trim()
    ));
  }
  Ok(output.stdout.trim_end().to_owned())
}

#[cfg(test)]
#[path = "tests/hc_results_test.rs"]
mod tests;
