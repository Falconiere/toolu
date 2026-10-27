//! `toolu code-review`, black-box: a bad effort is a usage error, and `run` fails
//! closed until the review pipeline lands (#661). The help text is pinned by the
//! generated `docs/cli/code-review.md`, which `cargo xtask docs-cli --check` compares.

use std::process::{Command, Output};

fn toolu(args: &[&str]) -> std::io::Result<Output> {
  Command::new(env!("CARGO_BIN_EXE_toolu"))
    .args(args)
    .output()
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
