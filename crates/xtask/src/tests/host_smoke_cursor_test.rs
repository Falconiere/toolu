use std::ffi::OsStr;
use std::path::Path;

use super::{Cursor, argv};
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_steps::{Driver, Step};

#[test]
fn a_session_runs_headless_with_both_plugins_from_the_checkout() {
  let words = argv(Path::new("/bin/cursor-agent"), Path::new("/repo"), "do it");
  assert_eq!(words[0], "/bin/cursor-agent");
  assert!(words.contains(&"-p".to_owned()) && words.contains(&"--trust".to_owned()));
  let dirs: Vec<&String> = words
    .iter()
    .zip(words.iter().skip(1))
    .filter(|(flag, _)| flag.as_str() == "--plugin-dir")
    .map(|(_, dir)| dir)
    .collect();
  assert_eq!(dirs, ["/repo/plugins/toolu", "/repo/plugins/ts-quality"]);
  assert_eq!(words.last().map(String::as_str), Some("do it"));
}

#[test]
fn the_driver_stages_first_and_names_itself_cursor() {
  let driver = Cursor {
    exe: "/bin/cursor-agent".into(),
    repo: "/repo".into(),
  };
  assert_eq!(driver.name(), "cursor");
  assert!(driver.prompts().stages_first);
  let step = Step {
    name: "deny",
    tools: "",
    prompt: "p",
  };
  assert_eq!(
    driver
      .argv(&step, Path::new("/p"))
      .last()
      .map(String::as_str),
    Some("p")
  );
}

#[test]
fn a_machine_without_cursor_agent_on_path_is_refused_by_name() {
  let dir = tempfile::tempdir().unwrap();
  let env = ChildEnv::new(
    Path::new("/bin/toolu"),
    OsStr::new("/nonexistent-dir"),
    &dir.path().join("shims"),
  )
  .unwrap();
  assert_eq!(
    env.tool("cursor-agent").unwrap_err(),
    "`cursor-agent` is not on PATH"
  );
}
