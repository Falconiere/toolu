//! `toolu epic job`: one command under the #421 managed-job runner, waiting
//! for admission and printing a bounded tail of its log.

use std::path::Path;
use std::time::{Duration, SystemTime};

use toolu_engine::resources::binding::{ResourceBinding, resource_binding};
use toolu_engine::resources::job_logs;
use toolu_engine::resources::jobs::{Admission, JobWait, refusal, run_managed_job_waiting};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::process::Spec;

use crate::journal::{self, Record};
use crate::paths::Paths;
use crate::waiting_jobs;

/// Lines of the merged log printed after a failure.
const FAIL_TAIL: usize = 80;
/// Lines of stdout printed after a success before the rest is left in the log.
const OK_TAIL: usize = 40;
/// How much of the log's end is read for the tail.
const TAIL_BYTES: usize = 64 * 1024;

/// Run `argv` in `cwd`'s bound worktree, waiting up to `bound` for admission
/// and retrying every `poll`; a zero bound refuses at once. The engine does
/// not need to be up.
pub(crate) fn run_job_with(
  argv: &[String],
  cwd: &Path,
  bound: Duration,
  poll: Duration,
) -> Outcome {
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
  let mut notes: Vec<String> = Vec::new();
  let mut observe = |admission: Admission<'_>| {
    let note = match admission {
      Admission::Attempt => admit(&binding, argv)
        .err()
        .map(|journal| format!("job not journaled: {journal}")),
      Admission::Refused(reason) => journal_refusal(&binding, reason),
    };
    notes.extend(note);
  };
  let mut wait = JobWait {
    bound,
    poll,
    observe: Some(&mut observe),
  };
  let mut spec = Spec::new(Vec::<String>::new());
  spec.timeout = Duration::from_secs(3600);
  spec.cwd = Some(binding.worktree.clone());
  let result = run_managed_job_waiting(argv, &binding, &spec, &mut wait);
  outcome(&binding, argv, result, notes.last())
}

/// The verb's outcome: bounded stdout, the failure tail, or the runner's
/// error with the last journal note. Anything but a final refusal clears the
/// waiting marker.
fn outcome(
  binding: &ResourceBinding,
  argv: &[String],
  result: Result<toolu_runtime::process::Output, String>,
  note: Option<&String>,
) -> Outcome {
  let refused = result.as_ref().is_err_and(|err| refusal(err).is_some());
  if !refused {
    waiting_jobs::clear(binding);
  }
  match result {
    Ok(output) if output.exit_code == 0 => Outcome {
      exit: Exit::Success,
      stdout: Some(bounded_stdout(binding, argv, &output.stdout)),
      stderr: note.cloned(),
    },
    Ok(output) => {
      let tail = log_tail(binding, argv, FAIL_TAIL);
      Outcome::failed(
        Exit::Failure,
        format!(
          "toolu epic job: command exited {}\n{tail}",
          output.exit_code
        ),
      )
    }
    Err(err) => {
      let mut message = format!("toolu epic job: {err}");
      if let Some(note) = note {
        message.push_str("; ");
        message.push_str(note);
      }
      Outcome::failed(Exit::Failure, message)
    }
  }
}

/// `stdout` whole when it is `OK_TAIL` lines or fewer, else its last
/// `OK_TAIL` lines and where the rest is.
fn bounded_stdout(binding: &ResourceBinding, argv: &[String], stdout: &str) -> String {
  if stdout.lines().nth(OK_TAIL).is_none() {
    return stdout.to_owned();
  }
  format!(
    "{}full log: {}",
    last_lines(stdout, OK_TAIL),
    log_path(binding, argv)
  )
}

/// The last `lines` of the merged log, then where the whole log is. The read
/// starts mid-line when the log is longer than `TAIL_BYTES`, so that leading
/// fragment is dropped.
fn log_tail(binding: &ResourceBinding, argv: &[String], lines: usize) -> String {
  match newest_log(binding, argv) {
    Ok(entry) => match job_logs::read(&entry, None, TAIL_BYTES) {
      Ok(chunk) => {
        let text = String::from_utf8_lossy(&chunk.bytes);
        let whole = if chunk.offset == 0 {
          text.as_ref()
        } else {
          text.split_once('\n').map_or("", |(_, rest)| rest)
        };
        format!(
          "{}full log: {}",
          last_lines(whole, lines),
          entry.path().display()
        )
      }
      Err(err) => format!("full log: unavailable ({err})"),
    },
    Err(err) => format!("full log: unavailable ({err})"),
  }
}

fn log_path(binding: &ResourceBinding, argv: &[String]) -> String {
  match newest_log(binding, argv) {
    Ok(entry) => entry.path().display().to_string(),
    Err(err) => format!("unavailable ({err})"),
  }
}

/// The newest log of this key written for `argv`.
// shortcut: two concurrent jobs of one worktree with the same argv (needs
// maxJobs above 1) can swap tails; thread the lease token through the runner
// if that ever matters.
fn newest_log(binding: &ResourceBinding, argv: &[String]) -> Result<job_logs::Entry, String> {
  let label = argv.join(" ");
  job_logs::list(&binding.root, &binding.key)?
    .into_iter()
    .filter(|entry| entry.label == label)
    .max_by(|a, b| a.started_at.cmp(&b.started_at))
    .ok_or_else(|| "no log recorded for this command".to_owned())
}

/// `tail -n lines` of `text`, newline-terminated.
fn last_lines(text: &str, lines: usize) -> String {
  let all: Vec<&str> = text.lines().collect();
  let kept = all.get(all.len().saturating_sub(lines)..).unwrap_or(&[]);
  let mut tail = kept.join("\n");
  if !tail.is_empty() {
    tail.push('\n');
  }
  tail
}

/// Journal a refusal (`job-wait`, `capacity` or `pressure`) and mark the
/// worktree waiting (#451); a journal failure is returned as a note rather
/// than dropped.
fn journal_refusal(binding: &ResourceBinding, reason: &str) -> Option<String> {
  let _marked = waiting_jobs::mark(binding, SystemTime::now());
  refused(binding, reason)
    .err()
    .map(|journal| format!("job-wait not journaled: {journal}"))
}

fn admit(binding: &ResourceBinding, argv: &[String]) -> Result<(), String> {
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
fn refused(binding: &ResourceBinding, reason: &str) -> Result<(), String> {
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
