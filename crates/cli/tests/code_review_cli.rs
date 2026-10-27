//! `toolu code-review`, black-box: the help text is pinned, a bad effort is a
//! usage error, and `run` fails closed until the review pipeline lands (#661).

use std::process::{Command, Output};

fn toolu(args: &[&str]) -> std::io::Result<Output> {
  Command::new(env!("CARGO_BIN_EXE_toolu"))
    .args(args)
    .output()
}

#[test]
fn the_help_matches_its_snapshot() {
  let mut settings = insta::Settings::clone_current();
  settings.set_snapshot_path("fixtures");
  settings.set_prepend_module_to_snapshot(false);
  for (name, args) in [
    ("code_review_help", &["code-review", "--help"][..]),
    ("code_review_run_help", &["code-review", "run", "--help"]),
  ] {
    let output = toolu(args).unwrap();
    assert_eq!(output.status.code(), Some(0), "{args:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    settings.bind(|| insta::assert_snapshot!(name, text));
  }
}

#[test]
fn an_unknown_effort_exits_64_with_nothing_on_stdout() {
  let output = toolu(&["code-review", "run", "--effort", "huge"]).unwrap();
  assert_eq!(output.status.code(), Some(64));
  assert!(output.stdout.is_empty(), "{:?}", output.stdout);
}

#[test]
fn run_fails_closed_with_exit_69_and_a_reason() {
  let output = toolu(&["code-review", "run", "--path", "src/lib.rs"]).unwrap();
  assert_eq!(output.status.code(), Some(69));
  assert!(output.stdout.is_empty(), "{:?}", output.stdout);
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert!(
    stderr.starts_with("code-review: unconfigured: the review pipeline is not built yet"),
    "{stderr}"
  );
}
