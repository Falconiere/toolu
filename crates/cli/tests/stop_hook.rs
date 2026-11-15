//! Stop responses from the real binary and real detached-job files.

use serde_json::json;

#[test]
fn cursor_stop_emits_a_followup_once() {
  let dir = tempfile::tempdir().unwrap();
  let project = dir.path().join("project");
  std::fs::create_dir(&project).unwrap();
  let project = std::fs::canonicalize(project).unwrap();
  assert!(
    std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(&project)
      .status()
      .unwrap()
      .success()
  );
  let root = dir.path().join("resources");
  let plugin_root = std::fs::canonicalize(
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu"),
  )
  .unwrap();
  let record =
    toolu_engine::resources::job_results::create(&root, "", &project, "running").unwrap();
  let stop = || {
    assert_cmd::Command::new(env!("CARGO_BIN_EXE_toolu"))
      .args(["hook", "stop", "--event", "Stop", "--plugin-root", ""])
      .env_clear()
      .env("PATH", std::env::var("PATH").unwrap())
      .env("HOME", dir.path().join("home"))
      .env("CURSOR_VERSION", "2026.11.13")
      .env("CURSOR_PLUGIN_ROOT", &plugin_root)
      .env("TOOLU_RESOURCE_HOME", &root)
      .current_dir(&project)
      .write_stdin("{}")
      .output()
      .unwrap()
  };
  let first = stop();
  assert_eq!(first.status.code(), Some(0));
  assert_eq!(String::from_utf8(first.stdout).unwrap(), json!({
    "followup_message": format!("detached job {} is pending or has an unread result; check it before stopping", record.id)
  }).to_string() + "\n");
  let second = stop();
  assert_eq!(second.status.code(), Some(0));
  assert_eq!(second.stdout, Vec::<u8>::new());
  assert_eq!(second.stderr, Vec::<u8>::new());
}

#[test]
fn claude_stop_blocks_a_finished_result_but_ignores_a_foreign_one() {
  let dir = tempfile::tempdir().unwrap();
  let project = dir.path().join("project");
  std::fs::create_dir(&project).unwrap();
  let project = std::fs::canonicalize(project).unwrap();
  assert!(
    std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(&project)
      .status()
      .unwrap()
      .success()
  );
  let root = dir.path().join("resources");
  let foreign =
    toolu_engine::resources::job_results::create(&root, "", &dir.path().join("other"), "foreign")
      .unwrap();
  let stop = || {
    assert_cmd::Command::new(env!("CARGO_BIN_EXE_toolu"))
      .args(["hook", "stop", "--event", "Stop"])
      .env_clear()
      .env("PATH", std::env::var("PATH").unwrap())
      .env("HOME", dir.path().join("home"))
      .env("CLAUDE_PROJECT_DIR", &project)
      .env("TOOLU_RESOURCE_HOME", &root)
      .current_dir(&project)
      .write_stdin("{}")
      .output()
      .unwrap()
  };
  assert_eq!(stop().status.code(), Some(0));
  let mut done = toolu_engine::resources::job_results::create(&root, "", &project, "done").unwrap();
  done.state = "done".to_owned();
  done.exit_code = Some(1);
  toolu_engine::resources::job_results::update(&root, &done).unwrap();
  let first = stop();
  assert_eq!(first.status.code(), Some(2));
  assert!(String::from_utf8(first.stderr).unwrap().contains(&done.id));
  assert_eq!(stop().status.code(), Some(0));
  assert_eq!(
    toolu_engine::resources::job_results::claim_finished(&root, &project).unwrap()[0].id,
    done.id
  );
  assert!(
    toolu_engine::resources::job_results::get(&root, &foreign.id)
      .unwrap()
      .unwrap()
      .delivered_at
      .is_none()
  );
}
