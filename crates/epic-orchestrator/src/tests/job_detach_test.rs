//! The detached copy path over a real bound worktree.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use toolu_engine::resources::binding::bind_worktree;
use toolu_engine::resources::job_results;
use toolu_protocol::exit::Exit;

use super::{run_detached, run_result};

fn bound(tmp: &Path) -> (PathBuf, PathBuf) {
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
  bind_worktree(&root, &work, "detached", &work.display().to_string()).expect("bind");
  (work, root)
}

fn argv(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn the_copy_records_done_with_the_exit_code_and_the_failure_tail() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path());
  let record = job_results::create(&root, "detached", &work, "sh -c echo hi; exit 3").unwrap();
  let outcome = run_result(
    &record.id,
    &argv(&["sh", "-c", "echo hi; exit 3"]),
    &work,
    Duration::from_secs(30),
    Duration::from_millis(50),
  );
  assert_eq!(outcome.exit, Exit::Failure, "{outcome:?}");
  let done = job_results::get(&root, &record.id).unwrap().unwrap();
  assert_eq!(
    (done.state.as_str(), done.exit_code),
    ("done", Some(3)),
    "{done:?}"
  );
  assert!(done.output.contains("command exited 3"), "{}", done.output);
  assert!(done.finished_at.is_some() && done.delivered_at.is_none());
  assert_eq!(job_results::claim_finished(&root, &work).unwrap().len(), 1);
}

#[test]
fn an_unknown_result_and_an_unbound_detach_run_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path());
  let unknown = run_result(
    "no-such-id",
    &argv(&["true"]),
    &work,
    Duration::ZERO,
    Duration::ZERO,
  );
  assert_eq!(unknown.exit, Exit::Failure);
  assert!(
    unknown
      .stderr
      .as_deref()
      .is_some_and(|text| text.contains("unknown job result no-such-id")),
    "{unknown:?}"
  );
  let unbound = tmp.path().join("unbound");
  std::fs::create_dir_all(&unbound).expect("unbound");
  let detached = run_detached(&argv(&["true"]), &unbound, Duration::ZERO, Duration::ZERO);
  assert_eq!(detached.exit, Exit::Failure);
  assert!(!root.join("jobs/results").exists() || job_results::pending(&root, &work).unwrap().is_empty());
}
