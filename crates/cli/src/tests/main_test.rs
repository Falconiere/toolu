use std::cell::Cell;

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

use super::{Context, VERSION, run, tree};

/// The words of `line`, as the shell would split a simple command line.
pub(crate) fn words(line: &str) -> Vec<String> {
  line.split_whitespace().map(str::to_owned).collect()
}

fn no_stdin() -> std::io::Result<String> {
  toolu_protocol::stdin::read_all(std::io::empty())
}

fn no_exe() -> Option<std::path::PathBuf> {
  None
}

/// A context with no standard input and no known executable path.
pub(crate) fn context() -> Context<'static> {
  Context {
    exe: &no_exe,
    stdin: &no_stdin,
    env: None,
  }
}

/// Run `line` with a tree builder that counts its calls.
fn counted(line: &str) -> (Outcome, usize) {
  let calls = Cell::new(0);
  let builder = || {
    calls.set(calls.get() + 1);
    tree::command()
  };
  let outcome = run(&words(line), &context(), &builder);
  (outcome, calls.get())
}

#[test]
fn the_fast_path_never_builds_the_tree() {
  for line in [
    "hook pre-tools --event PreToolUse --plugin-root /p",
    "hook session-start --event SessionStart",
    "jev hook session-start --event SessionStart",
    "pr-babysit hook check-toolu",
    "--hook-protocol",
  ] {
    let (_, calls) = counted(line);
    assert_eq!(calls, 0, "{line}");
  }
  let (outcome, _) = counted("--hook-protocol");
  assert_eq!(outcome, Outcome::data("1".to_owned()));
}

#[test]
fn a_namespace_word_hook_line_runs_as_its_plugin_through_clap() {
  let (fast, fast_calls) = counted("toolu-review hook x --event Stop");
  let (clap, clap_calls) = counted("review hook x --event Stop");
  let (quiet, _) = counted("--quiet review hook x --event Stop");
  assert_eq!((fast_calls, clap_calls), (0, 1));
  assert_eq!(clap, fast);
  assert_eq!(quiet.exit, fast.exit);
}

#[test]
fn a_repeated_hook_flag_is_a_usage_error_with_or_without_global_flags() {
  for line in [
    "hook x --event Stop --event PreToolUse",
    "--quiet hook x --event Stop --event PreToolUse",
  ] {
    let (outcome, calls) = counted(line);
    assert_eq!((outcome.exit, calls), (Exit::Usage, 1), "{line}");
  }
}

#[test]
fn version_builds_the_tree_once_and_prints_one_line() {
  let (outcome, calls) = counted("--version");
  assert_eq!(calls, 1);
  assert_eq!(outcome, Outcome::data(format!("toolu {VERSION}")));
}

#[test]
fn json_version_is_one_document_with_the_hook_protocol() {
  let (outcome, _) = counted("--json --version");
  let doc: Value = serde_json::from_str(&outcome.stdout.unwrap()).unwrap();
  assert_eq!(doc["version"], VERSION);
  assert_eq!(doc["hookProtocol"], 1);
}

#[test]
fn a_malformed_hook_line_goes_through_clap_and_is_a_usage_error() {
  let (outcome, calls) = counted("hook pre-tools --bogus x");
  assert_eq!(calls, 1);
  assert_eq!(outcome.exit, Exit::Usage);
  assert!(outcome.stderr.unwrap().contains("--bogus"));
  assert_eq!(outcome.stdout, None);
}

#[test]
fn the_version_is_the_workspace_version() {
  let manifest = include_str!("../../../../Cargo.toml");
  assert!(manifest.contains(&format!("version = \"{VERSION}\"")));
}

#[test]
fn a_hook_run_goes_through_the_hook_module() {
  let (outcome, _) = counted("hook no-such-hook --event PreToolUse");
  assert_eq!(outcome.exit, Exit::Blocked);
  assert!(
    outcome
      .stderr
      .unwrap()
      .starts_with("blocked: toolu plugin: toolu ")
  );
}
