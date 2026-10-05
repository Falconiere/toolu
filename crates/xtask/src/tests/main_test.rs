use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use super::{TASKS, run};

/// Write an executable through a child `sh`, so no descriptor open for writing
/// lives in this multi-threaded test process: a sibling test's fork could
/// inherit it and make the launcher's `exec` fail with `ETXTBSY`.
pub(crate) fn install(path: &Path, text: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  let mut child = Command::new("/bin/sh")
    .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
    .arg(path)
    .stdin(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all(text.as_bytes())
    .unwrap();
  assert!(child.wait().unwrap().success());
}

fn words(args: &[&str]) -> Vec<String> {
  args.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn every_task_is_listed_in_the_usage() {
  for (name, _) in TASKS {
    assert!(super::USAGE.contains(name), "{name}");
  }
}

#[test]
fn unknown_tasks_bad_options_and_no_task_exit_2() {
  for args in [&[][..], &["nope"][..], &["guardrails", "--x", "y"][..]] {
    assert_eq!(run(&words(args)), ExitCode::from(2), "{args:?}");
  }
}

#[test]
fn a_clean_task_exits_0() {
  assert_eq!(run(&words(&["check-reach"])), ExitCode::SUCCESS);
}
