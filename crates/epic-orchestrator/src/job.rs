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
    Ok(output) if output.exit_code == 0 => Outcome::data(output.stdout),
    Ok(output) => Outcome::failed(
      Exit::Failure,
      format!("toolu epic job: command exited {}", output.exit_code),
    ),
    Err(err) => Outcome::failed(Exit::Failure, format!("toolu epic job: {err}")),
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

#[cfg(test)]
#[path = "tests/job_test.rs"]
mod tests;
