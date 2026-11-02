use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use toolu_engine::resources::binding::bind_worktree;
use toolu_engine::resources::jobs::{JOB_WAIT_BOUND, JOB_WAIT_POLL};
use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease, release_lease};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::json::ordered::Ordered;

use super::run_job_with;
use crate::journal;
use crate::paths::Paths;

/// A bound git worktree and its resource root under `tmp`.
fn bound(tmp: &Path, key: &str) -> (PathBuf, PathBuf) {
  let work = tmp.join("work");
  std::fs::create_dir_all(&work).expect("work");
  let init = Command::new("git")
    .args(["init", "-q"])
    .current_dir(&work)
    .status()
    .expect("git init");
  assert!(init.success());
  let root = tmp.join("resources");
  std::fs::create_dir_all(&root).expect("root");
  bind_worktree(&root, &work, key, &work.display().to_string()).expect("bind");
  (work, root)
}

/// Occupy the only job slot; the lease token.
fn hold(root: &Path) -> String {
  let occupied = LeaseRequest {
    kind: LeaseKind::Job,
    key: "occupied".to_owned(),
    state_dir: "other".to_owned(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(root, &occupied, SystemTime::now()).expect("lease");
  let Some(Ordered::String(token)) = lease.get("token") else {
    panic!("lease without a token: {lease:?}");
  };
  token.clone()
}

fn admissions(root: &Path) -> Vec<(String, String)> {
  journal::tail(&Paths::at(root).journal_dir(), SystemTime::now())
    .expect("journal")
    .iter()
    .filter(|row| row.kind == "admission")
    .map(|row| (row.name.clone(), row.note.clone()))
    .collect()
}

fn run(argv: &[&str], work: &Path) -> Outcome {
  let argv: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
  run_job_with(&argv, work, JOB_WAIT_BOUND, JOB_WAIT_POLL)
}

#[test]
fn epic_job() {
  let tmp = tempfile::tempdir().expect("temp");
  let unbound = tmp.path().join("unbound");
  std::fs::create_dir_all(&unbound).expect("unbound");
  let missing = run(&["true"], &unbound);
  assert_eq!(missing.exit, Exit::Failure);
  assert_eq!(
    missing.stderr.as_deref(),
    Some("worktree has no epic resource binding")
  );
  let (work, root) = bound(tmp.path(), "one");
  let ran = run(&["true"], &work);
  assert_eq!(ran.exit, Exit::Success, "{ran:?}");
  let paths = Paths::at(&root);
  assert_eq!(admissions(&root), [("job".to_owned(), "true".to_owned())]);
  assert!(!paths.lock().exists());
}

#[test]
fn a_waiting_job_journals_each_attempt_and_refusal_and_clears_its_marker() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "i-7");
  let token = hold(&root);
  let releasing = root.clone();
  let release = std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(300));
    release_lease(&releasing, &token).expect("release");
  });
  let admitted = run_job_with(
    &["true".to_owned()],
    &work,
    Duration::from_secs(5),
    Duration::from_millis(50),
  );
  release.join().expect("release thread");
  assert_eq!(admitted.exit, Exit::Success, "{admitted:?}");
  let rows = admissions(&root);
  assert_eq!(
    rows[..2],
    [
      ("job".to_owned(), "true".to_owned()),
      ("job-wait".to_owned(), "capacity".to_owned())
    ]
  );
  assert_eq!(rows.last(), Some(&("job".to_owned(), "true".to_owned())));
  assert!(!root.join("waiting-jobs/i-7.json").exists());
}

#[test]
fn a_refused_job_with_no_wait_journals_its_wait_reason_as_before() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "i-7");
  hold(&root);
  let refused = run_job_with(&["true".to_owned()], &work, Duration::ZERO, Duration::ZERO);
  assert_eq!(refused.exit, Exit::Failure, "{refused:?}");
  assert_eq!(
    refused.stderr.as_deref(),
    Some("toolu epic job: job capacity exhausted (1)")
  );
  assert_eq!(
    admissions(&root),
    [
      ("job".to_owned(), "true".to_owned()),
      ("job-wait".to_owned(), "capacity".to_owned())
    ]
  );
  assert!(root.join("waiting-jobs/i-7.json").exists());
}

#[test]
fn a_refusal_the_journal_cannot_record_is_named_in_the_failure() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "i-9");
  hold(&root);
  std::fs::write(Paths::at(&root).journal_dir(), "not a directory").expect("block");
  let refused = run_job_with(&["true".to_owned()], &work, Duration::ZERO, Duration::ZERO);
  let message = refused.stderr.unwrap_or_default();
  assert!(
    message.starts_with("toolu epic job: job capacity exhausted (1); job-wait not journaled: "),
    "{message}"
  );
}

#[test]
fn a_failed_command_prints_the_last_eighty_log_lines_and_the_log_path() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, _root) = bound(tmp.path(), "tail");
  let failed = run(&["sh", "-c", "seq 1 100; echo boom >&2; exit 3"], &work);
  assert_eq!(failed.exit, Exit::Failure);
  let message = failed.stderr.unwrap_or_default();
  assert!(
    message.starts_with("toolu epic job: command exited 3\n"),
    "{message}"
  );
  let (tail, path) = message.rsplit_once("full log: ").expect("log path");
  // The two streams land in the log in read order, so `boom` may sit at
  // either end; the tail is the last 80 of the 101 lines either way.
  let lines: Vec<&str> = tail.lines().skip(1).collect();
  assert_eq!(lines.len(), 80, "{message}");
  assert!(
    lines.contains(&"100") && !lines.contains(&"20"),
    "{message}"
  );
  assert!(Path::new(path.trim()).is_file(), "{path}");
  assert!(
    std::fs::read_to_string(path.trim())
      .expect("log")
      .contains("boom\n")
  );
}

#[test]
fn a_long_success_keeps_its_last_forty_lines_and_a_short_one_prints_whole() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, _root) = bound(tmp.path(), "ok");
  let long = run(&["seq", "1", "100"], &work);
  assert_eq!(long.exit, Exit::Success);
  let stdout = long.stdout.unwrap_or_default();
  assert!(stdout.starts_with("61\n62\n"), "{stdout}");
  assert!(stdout.contains("\n100\nfull log: "), "{stdout}");
  let short = run(&["seq", "1", "40"], &work);
  let stdout = short.stdout.unwrap_or_default();
  assert_eq!(stdout.lines().count(), 40, "{stdout}");
  assert!(
    stdout.starts_with("1\n2\n") && stdout.ends_with("\n40\n"),
    "{stdout}"
  );
  assert!(!stdout.contains("full log"), "{stdout}");
}

#[test]
fn a_job_whose_attempt_cannot_be_journaled_still_runs_and_says_so() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "i-3");
  std::fs::write(Paths::at(&root).journal_dir(), "not a directory").expect("block");
  let ran = run(&["printf", "ok"], &work);
  assert_eq!(ran.exit, Exit::Success, "{ran:?}");
  assert_eq!(ran.stdout.as_deref(), Some("ok"));
  assert!(
    ran
      .stderr
      .as_deref()
      .is_some_and(|note| note.starts_with("job not journaled: ")),
    "{ran:?}"
  );
}

#[test]
fn a_run_failure_after_a_wait_clears_the_waiting_marker() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "i-5");
  let token = hold(&root);
  std::fs::create_dir_all(root.join("jobs")).expect("jobs");
  std::fs::write(root.join("jobs/logs"), "not a directory").expect("block logs");
  let releasing = root.clone();
  let release = std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(300));
    release_lease(&releasing, &token).expect("release");
  });
  let failed = run_job_with(
    &["true".to_owned()],
    &work,
    Duration::from_secs(5),
    Duration::from_millis(50),
  );
  release.join().expect("release thread");
  assert_eq!(failed.exit, Exit::Failure, "{failed:?}");
  assert!(
    failed
      .stderr
      .as_deref()
      .is_some_and(|m| m.contains("jobs/logs")),
    "{failed:?}"
  );
  assert!(!root.join("waiting-jobs/i-5.json").exists());
}
