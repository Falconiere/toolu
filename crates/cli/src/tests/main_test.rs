use super::{Context, Outcome, VERSION, run};

fn words(line: &str) -> Vec<String> {
  line.split_whitespace().map(str::to_owned).collect()
}

fn no_stdin() -> String {
  String::new()
}

fn no_exe() -> Option<std::path::PathBuf> {
  None
}

fn context() -> Context<'static> {
  Context {
    exe: &no_exe,
    stdin: &no_stdin,
  }
}

#[test]
fn version_and_hook_protocol_print_one_line() {
  assert_eq!(
    run(&words("--version"), &context()),
    Outcome {
      code: 0,
      stdout: Some(format!("toolu {VERSION}")),
      stderr: None
    }
  );
  assert_eq!(
    run(&words("--hook-protocol"), &context()).stdout.as_deref(),
    Some("1")
  );
}

#[test]
fn the_version_is_the_workspace_version() {
  let manifest = include_str!("../../../../Cargo.toml");
  assert!(manifest.contains(&format!("version = \"{VERSION}\"")));
}

#[test]
fn unknown_argv_exits_64_with_usage() {
  for line in [
    "--nope",
    "",
    "--version extra",
    "hook",
    "toolu hook --event X",
  ] {
    let outcome = run(&words(line), &context());
    assert_eq!(outcome.code, 64, "{line}");
    assert!(
      outcome.stderr.unwrap().contains("usage: toolu --version"),
      "{line}"
    );
    assert_eq!(outcome.stdout, None);
  }
}

#[test]
fn a_hook_run_goes_through_the_hook_module() {
  let outcome = run(&words("hook pre-tools --event PreToolUse"), &context());
  assert_eq!(outcome.code, 2);
  assert!(
    outcome
      .stderr
      .unwrap()
      .starts_with("blocked: toolu plugin: toolu ")
  );
}
