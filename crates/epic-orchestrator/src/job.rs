//! `toolu epic job`: one command under the #421 managed-job runner.

use std::path::Path;
use std::time::{Duration, SystemTime};

use toolu_engine::resources::binding::resource_binding;
use toolu_engine::resources::jobs::run_managed_job;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::process::Spec;

use crate::journal::{self, Record};
use crate::paths::Paths;
use crate::waiting_jobs;

/// Run `argv` in `cwd`'s bound worktree. The engine does not need to be up.
pub(crate) fn run_job(argv: &[String], cwd: &Path) -> Outcome {
  let binding = match resource_binding(cwd) {
    Ok(Some(binding)) => binding,
    Ok(None) => {
      return Outcome::failed(
        Exit::Failure,
        "worktree has no epic resource binding".to_owned(),
      );
    }
    Err(err) => return Outcome::failed(Exit::Failure, err),
  };
  if let Err(err) = admit(&binding, argv) {
    return Outcome::failed(Exit::Failure, format!("toolu epic job: {err}"));
  }
  let mut spec = Spec::new(Vec::<String>::new());
  spec.timeout = Duration::from_secs(3600);
  spec.cwd = Some(binding.worktree.clone());
  match run_managed_job(argv, &binding, &spec) {
    Ok(output) if output.exit_code == 0 => {
      waiting_jobs::clear(&binding);
      Outcome::data(output.stdout)
    }
    Ok(output) => {
      waiting_jobs::clear(&binding);
      Outcome::failed(
        Exit::Failure,
        format!("toolu epic job: command exited {}", output.exit_code),
      )
    }
    Err(err) => Outcome::failed(Exit::Failure, failure(&binding, &err)),
  }
}

/// The message of a job that did not run. A refusal (`capacity` or
/// `pressure`) is marked waiting and journaled as `job-wait`; a journal
/// failure is named in the message rather than dropped (#451).
fn failure(binding: &toolu_engine::resources::binding::ResourceBinding, err: &str) -> String {
  let message = format!("toolu epic job: {err}");
  let reason = if err.starts_with("resource hold:") {
    "pressure"
  } else if err.starts_with("job capacity exhausted") {
    "capacity"
  } else {
    return message;
  };
  let _marked = waiting_jobs::mark(binding, SystemTime::now());
  match refused(binding, reason) {
    Ok(()) => message,
    Err(journal) => format!("{message}; job-wait not journaled: {journal}"),
  }
}

fn admit(
  binding: &toolu_engine::resources::binding::ResourceBinding,
  argv: &[String],
) -> Result<(), String> {
  let paths = Paths::at(&binding.root);
  let record = Record::new("admission", "job", &binding.key, "", &argv.join(" "));
  journal::append(
    &paths.journal_dir(),
    &paths.journal_lock(),
    &record,
    SystemTime::now(),
  )?;
  Ok(())
}

/// Journal a refused attempt (`job-wait`), `capacity` or `pressure` (#451).
fn refused(
  binding: &toolu_engine::resources::binding::ResourceBinding,
  reason: &str,
) -> Result<(), String> {
  let paths = Paths::at(&binding.root);
  let record = Record::new("admission", "job-wait", &binding.key, "", reason);
  journal::append(
    &paths.journal_dir(),
    &paths.journal_lock(),
    &record,
    SystemTime::now(),
  )
  .map(|_| ())
}

#[cfg(test)]
#[path = "tests/job_test.rs"]
mod tests;
