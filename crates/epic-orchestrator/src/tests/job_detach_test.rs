//! The detached copy path over a real bound worktree.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use toolu_engine::resources::binding::bind_worktree;
use toolu_engine::resources::job_results;
use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease, release_lease};
use toolu_protocol::exit::Exit;
use toolu_runtime::json::ordered::Ordered;

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
fn a_refused_copy_stays_waiting_until_it_acquires_the_lease() {
  let tmp = tempfile::tempdir().expect("temp");
  let (work, root) = bound(tmp.path());
  std::fs::write(root.join("policy.json"), r#"{"maxAgents":1,"maxJobs":1}"#).unwrap();
  let occupied = LeaseRequest {
    kind: LeaseKind::Job,
    key: "occupied".to_owned(),
    state_dir: "other".to_owned(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(&root, &occupied, SystemTime::now()).unwrap();
  let Some(Ordered::String(token)) = lease.get("token") else {
    panic!("lease without token: {lease:?}");
  };
  let record = job_results::create(&root, "detached", &work, "true").unwrap();
  let copy_work = work.clone();
  let id = record.id.clone();
  let copy = std::thread::spawn(move || {
    run_result(
      &id,
      &argv(&["true"]),
      &copy_work,
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
  release_lease(&root, token).unwrap();
  assert_eq!(copy.join().unwrap().exit, Exit::Success);
  assert_eq!(
    job_results::get(&root, &record.id).unwrap().unwrap().state,
    "done"
  );
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
  assert!(
    !root.join("jobs/results").exists() || job_results::pending(&root, &work).unwrap().is_empty()
  );
}
