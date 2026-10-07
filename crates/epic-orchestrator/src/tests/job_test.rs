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
