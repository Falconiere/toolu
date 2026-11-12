//! The detached copy path over a real bound worktree.

use std::time::{Duration, Instant};

use toolu_engine::resources::job_results;
use toolu_engine::resources::leases::release_lease;
use toolu_protocol::exit::Exit;

use super::{fail_early, run_detached, run_result};
use crate::job_fixture::{bound, hold};

fn argv(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn an_early_failure_names_a_result_write_error() {
  let tmp = tempfile::tempdir().expect("temp");
  let root = tmp.path().join("resources");
  let mut record = job_results::create(&root, "detached", tmp.path(), "true").unwrap();
  let path = root
    .join("jobs/results")
    .join(format!("{}.json", record.id));
  std::fs::remove_file(&path).unwrap();
  std::fs::create_dir(path).unwrap();
  let failed = fail_early(&root, &mut record, "spawn failed".to_owned());
  assert_eq!(failed.exit, Exit::Failure);
  let error = failed.stderr.unwrap();
  assert!(error.contains("spawn failed"), "{error}");
  assert!(error.contains("not recorded"), "{error}");
}

#[test]
fn the_copy_records_done_with_the_exit_code_and_the_failure_tail() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "detached");
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
fn a_refused_copy_stays_waiting_until_it_acquires_the_lease() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "detached");
  std::fs::write(root.join("policy.json"), r#"{"maxAgents":1,"maxJobs":1}"#).unwrap();
  let token = hold(&root);
  let record = job_results::create(&root, "detached", &work, "true").unwrap();
  let id = record.id.clone();
  let copy = std::thread::spawn(move || {
    run_result(
      &id,
      &argv(&["true"]),
      &work,
      Duration::from_secs(5),
      Duration::from_millis(20),
    )
  });
  let marker = root.join("waiting-jobs/detached.json");
  let deadline = Instant::now() + Duration::from_secs(2);
  while !marker.exists() {
    assert!(Instant::now() < deadline, "copy never attempted admission");
    std::thread::sleep(Duration::from_millis(20));
  }
  assert_eq!(
    job_results::get(&root, &record.id).unwrap().unwrap().state,
    "waiting"
  );
  release_lease(&root, &token).unwrap();
  assert_eq!(copy.join().unwrap().exit, Exit::Success);
  assert_eq!(
    job_results::get(&root, &record.id).unwrap().unwrap().state,
    "done"
  );
}

#[test]
fn an_unknown_result_and_an_unbound_detach_run_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path(), "detached");
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
  let detached = run_detached(&argv(&["true"]), &unbound, Duration::ZERO);
  assert_eq!(detached.exit, Exit::Failure);
  assert!(
    !root.join("jobs/results").exists() || job_results::pending(&root, &work).unwrap().is_empty()
  );
}
