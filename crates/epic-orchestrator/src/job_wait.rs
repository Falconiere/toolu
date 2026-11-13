//! One blocking read of a detached job result for the current worktree.

use std::path::Path;
use std::time::{Duration, Instant};

use toolu_engine::resources::{job_results, store::resource_home};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::{env::Env, git};

use crate::verbs::failed;

/// Wait for `id`, or the oldest pending result when it is empty.
pub(crate) fn run(env: &Env, cwd: &Path, id: &str, max_seconds: u64) -> Outcome {
  let Some(worktree) = git::toplevel(env, cwd) else {
    return failed("toolu epic job", "no Git worktree found");
  };
  let root = resource_home(env);
  let selected = if id.is_empty() {
    match job_results::pending(&root, &worktree) {
      Ok(records) => match records.first() {
        Some(record) => record.id.clone(),
        None => return Outcome::data(String::new()),
      },
      Err(err) => return failed("toolu epic job", &err),
    }
  } else {
    id.to_owned()
  };
  let started = Instant::now();
  let bound = Duration::from_secs(max_seconds);
  loop {
    let record = match job_results::get(&root, &selected) {
      Ok(Some(record)) if record.worktree == worktree.display().to_string() => record,
      Ok(Some(_)) => return failed("toolu epic job", "job result belongs to another worktree"),
      Ok(None) => return failed("toolu epic job", &format!("unknown job result {selected}")),
      Err(err) => return failed("toolu epic job", &err),
    };
    if record.delivered_at.is_some() {
      return failed(
        "toolu epic job",
        &format!("job result {selected} already delivered"),
      );
    }
    if record.state == "done" {
      return claim(&root, &worktree, &selected);
    }
    if started.elapsed() >= bound {
      return timeout(&root, &worktree);
    }
    std::thread::sleep(Duration::from_millis(100).min(bound.saturating_sub(started.elapsed())));
  }
}

fn claim(root: &Path, worktree: &Path, id: &str) -> Outcome {
  match job_results::claim_one(root, worktree, id) {
    Ok(Some(done)) => match done.exit_code {
      Some(0) => Outcome::data(done.output),
      Some(_) => Outcome::failed(Exit::Failure, done.output),
      None => failed("toolu epic job", "finished job has no exit code"),
    },
    Ok(None) => failed(
      "toolu epic job",
      &format!("job result {id} already delivered"),
    ),
    Err(err) => failed("toolu epic job", &err),
  }
}

fn timeout(root: &Path, worktree: &Path) -> Outcome {
  match job_results::pending(root, worktree) {
    Ok(records) => failed(
      "toolu epic job",
      &format!(
        "wait timed out; pending ids: {}",
        records
          .iter()
          .map(|record| record.id.as_str())
          .collect::<Vec<_>>()
          .join(", ")
      ),
    ),
    Err(err) => failed("toolu epic job", &err),
  }
}

#[cfg(test)]
#[path = "tests/job_wait_test.rs"]
mod tests;
