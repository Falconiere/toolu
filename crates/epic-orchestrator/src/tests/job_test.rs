use std::process::Command;

use toolu_engine::resources::binding::bind_worktree;
use toolu_protocol::exit::Exit;

use super::run_job;
use crate::journal;
use crate::paths::Paths;

#[test]
fn epic_job() {
  let tmp = tempfile::tempdir().expect("temp");
  let unbound = tmp.path().join("unbound");
  std::fs::create_dir_all(&unbound).expect("unbound");
  let missing = run_job(&["true".to_owned()], &unbound);
  assert_eq!(missing.exit, Exit::Failure);
  assert_eq!(
    missing.stderr.as_deref(),
    Some("worktree has no epic resource binding")
  );

  let work = tmp.path().join("work");
  std::fs::create_dir_all(&work).expect("work");
  let init = Command::new("git")
    .args(["init"])
    .current_dir(&work)
    .status()
    .expect("git init");
  assert!(init.success());
  let root = tmp.path().join("resources");
  std::fs::create_dir_all(&root).expect("root");
  let state = work.display().to_string();
  bind_worktree(&root, &work, "one", &state).expect("bind");
  let ran = run_job(&["true".to_owned()], &work);
  assert_eq!(ran.exit, Exit::Success, "{ran:?}");
  let paths = Paths::at(&root);
  let rows = journal::tail(&paths.journal_dir(), std::time::SystemTime::now()).expect("journal");
  assert!(
    rows
      .iter()
      .any(|row| row.kind == "admission" && row.name == "job")
  );
  assert!(!paths.lock().exists());
}

#[test]
fn a_refused_job_journals_its_wait_reason() {
  use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};
  let tmp = tempfile::tempdir().expect("temp");
  let work = tmp.path().join("work");
  std::fs::create_dir_all(&work).expect("work");
  let init = Command::new("git")
    .args(["init", "-q"])
    .current_dir(&work)
    .status();
  assert!(init.expect("git init").success());
  let root = tmp.path().join("resources");
  std::fs::create_dir_all(&root).expect("root");
  bind_worktree(&root, &work, "i-7", &work.display().to_string()).expect("bind");
  let occupied = LeaseRequest {
    kind: LeaseKind::Job,
    key: "occupied".to_owned(),
    state_dir: "other".to_owned(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  acquire_lease(&root, &occupied, std::time::SystemTime::now()).expect("lease");
  let refused = run_job(&["true".to_owned()], &work);
  assert_eq!(refused.exit, Exit::Failure, "{refused:?}");
  let rows = journal::tail(
    &Paths::at(&root).journal_dir(),
    std::time::SystemTime::now(),
  )
  .expect("journal");
  let admission: Vec<(&str, &str, &str)> = rows
    .iter()
    .filter(|row| row.kind == "admission")
    .map(|row| (row.name.as_str(), row.key.as_str(), row.note.as_str()))
    .collect();
  assert_eq!(
    admission,
    [("job", "i-7", "true"), ("job-wait", "i-7", "capacity")]
  );
}

#[test]
fn a_refusal_the_journal_cannot_record_is_named_in_the_failure() {
  use toolu_engine::resources::binding::ResourceBinding;
  let tmp = tempfile::tempdir().expect("temp");
  let binding = ResourceBinding {
    root: tmp.path().to_path_buf(),
    key: "i-9".to_owned(),
    state_dir: "state".to_owned(),
    worktree: tmp.path().to_path_buf(),
  };
  assert_eq!(
    super::failure(&binding, "spawn failed"),
    "toolu epic job: spawn failed"
  );
  std::fs::write(Paths::at(tmp.path()).journal_dir(), "not a directory").expect("block");
  let message = super::failure(&binding, "job capacity exhausted (1 of 1)");
  assert!(
    message
      .starts_with("toolu epic job: job capacity exhausted (1 of 1); job-wait not journaled: "),
    "{message}"
  );
}
