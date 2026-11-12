//! `toolu epic job --detach`: the launcher prints a job id at once; the
//! detached copy (`--result <id>`) runs the blocking job, owns its lease,
//! records the result, and announces it to the engine.

use std::path::Path;
use std::time::{Duration, SystemTime};

use toolu_engine::resources::job_results;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::invocation::current_exe;
use toolu_runtime::process::{Spec, spawn_orphan};
use toolu_state::time::iso_millis;

use crate::job::{binding_of, run_job_observed};

/// Record `record` as failed before it ever ran, and fail the launcher.
fn fail_early(root: &Path, record: &mut job_results::Record, message: String) -> Outcome {
  record.state = "done".to_owned();
  record.exit_code = Some(1);
  record.finished_at = Some(iso_millis(SystemTime::now()));
  record.output = message.clone();
  let _recorded = job_results::update(root, record);
  Outcome::failed(Exit::Failure, message)
}

/// Create the `waiting` record, relaunch this verb as its copy, print the id.
pub(crate) fn run_detached(
  argv: &[String],
  cwd: &Path,
  bound: Duration,
  poll: Duration,
) -> Outcome {
  let binding = match binding_of(cwd) {
    Ok(binding) => binding,
    Err(outcome) => return outcome,
  };
  let mut record = match job_results::create(
    &binding.root,
    &binding.key,
    &binding.worktree,
    &argv.join(" "),
  ) {
    Ok(record) => record,
    Err(err) => return Outcome::failed(Exit::Failure, err),
  };
  let Some(exe) = current_exe() else {
    return fail_early(
      &binding.root,
      &mut record,
      "cannot find the current executable".to_owned(),
    );
  };
  let mut relaunch = vec![
    exe.display().to_string(),
    "epic".to_owned(),
    "job".to_owned(),
    "--result".to_owned(),
    record.id.clone(),
  ];
  if bound.is_zero() {
    relaunch.push("--no-wait".to_owned());
  }
  let _poll = poll;
  relaunch.push("--".to_owned());
  relaunch.extend(argv.iter().cloned());
  let mut spec = Spec::new(relaunch);
  spec.cwd = Some(binding.worktree.clone());
  match spawn_orphan(&spec) {
    Ok(_) => Outcome::data(format!("{}\n", record.id)),
    Err(err) => fail_early(
      &binding.root,
      &mut record,
      format!("toolu epic job: cannot detach: {err:?}"),
    ),
  }
}

/// The detached copy: run the job and record its outcome in result `id`.
pub(crate) fn run_result(
  id: &str,
  argv: &[String],
  cwd: &Path,
  bound: Duration,
  poll: Duration,
) -> Outcome {
  let binding = match binding_of(cwd) {
    Ok(binding) => binding,
    Err(outcome) => return outcome,
  };
  let root = binding.root.clone();
  let mut record = match job_results::get(&root, id) {
    Ok(Some(record)) => record,
    Ok(None) => {
      return Outcome::failed(Exit::Failure, format!("unknown job result {id}"));
    }
    Err(err) => return Outcome::failed(Exit::Failure, err),
  };
  let mut running = false;
  let (outcome, code) = {
    let mut on_admitted = || {
      if !running {
        running = true;
        record.state = "running".to_owned();
        let _recorded = job_results::update(&root, &record);
      }
    };
    run_job_observed(argv, cwd, bound, poll, &mut on_admitted)
  };
  record.state = "done".to_owned();
  record.exit_code = Some(i64::from(code.unwrap_or(1)));
  record.finished_at = Some(iso_millis(SystemTime::now()));
  record.output = if outcome.exit == Exit::Success {
    outcome.stdout.clone().unwrap_or_default()
  } else {
    outcome.stderr.clone().unwrap_or_default()
  };
  if let Err(err) = job_results::update(&root, &record) {
    return Outcome::failed(Exit::Failure, format!("job result not recorded: {err}"));
  }
  job_results::announce(&root, &record.key);
  outcome
}

#[cfg(test)]
#[path = "tests/job_detach_test.rs"]
mod tests;
