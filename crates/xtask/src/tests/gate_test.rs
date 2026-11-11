use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{COLLECTED, STEPS, require, run, status};
use crate::Verdict;
use crate::options::Options;

fn options(root: PathBuf, only: &[&str]) -> Options {
  Options {
    root,
    only: only.iter().map(|step| (*step).to_owned()).collect(),
    ..Options::default()
  }
}

#[test]
fn every_step_has_an_inventory_free_name_and_unknown_steps_are_refused() {
  assert_eq!(
    STEPS,
    &[
      "gate-change",
      "fmt",
      "guardrails",
      "layers",
      "reach",
      "deny",
      "machete",
      "unused-pub",
      "hooks",
      "workflows",
      "jscpd",
      "web",
      "context-budget",
      "final-removal",
      "gate-coverage",
      "packaging",
      "ci-paths",
      "pack",
      "clippy",
      "docs-cli",
      "cli-compat",
      "rust-quality",
      "tests",
      "coverage",
      "docs",
      "bench",
    ]
  );
  assert_eq!(STEPS.len(), 26);
  assert_eq!(STEPS[COLLECTED - 1], "clippy");
  let err = run(&options(PathBuf::from("."), &["nope"])).unwrap_err();
  assert!(
    err.starts_with("unknown gate step nope; steps: gate-change, fmt"),
    "{err}"
  );
}

#[test]
fn context_budget_step_uses_the_task() {
  let mut opts = options(PathBuf::from("."), &["context-budget"]);
  opts.files.push(PathBuf::from("nope"));
  let err = run(&opts).unwrap_err();
  assert!(
    err.starts_with("usage: cargo xtask context-budget"),
    "{err}"
  );
}

#[test]
fn the_retired_typescript_steps_are_unknown() {
  for step in ["dist", "workspace", "portable-core"] {
    let err = run(&options(PathBuf::from("."), &[step])).unwrap_err();
    assert!(
      err.starts_with(&format!("unknown gate step {step}")),
      "{err}"
    );
  }
}

#[test]
fn bench_step_runs_the_deterministic_tier() {
  let dir = tempfile::tempdir().unwrap();
  let err = run(&options(dir.path().to_path_buf(), &["bench"])).unwrap_err();
  assert!(err.contains("queries file not found"), "{err}");
}

fn invalid_cargo_project(root: &Path) {
  std::fs::create_dir(root.join("src")).unwrap();
  std::fs::write(
    root.join("Cargo.toml"),
    "[package]\nname = \"gate-invalid\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
  )
  .unwrap();
  std::fs::write(root.join("src/main.rs"), "fn main( {\n").unwrap();
}

fn gate_process(root: &Path, only: &[&str]) -> Output {
  let repo = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
  let mut command = Command::new("cargo");
  command
    .args(["run", "--quiet", "-p", "xtask", "--", "gate", "--root"])
    .arg(root)
    .current_dir(repo);
  for step in only {
    command.args(["--only", step]);
  }
  command.output().unwrap()
}

#[test]
fn collected_failures_are_all_reported_before_build_steps() {
  let dir = tempfile::tempdir().unwrap();
  invalid_cargo_project(dir.path());
  let output = gate_process(dir.path(), &["fmt", "clippy", "docs"]);
  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
  assert!(stdout.contains("gate: fmt"), "{stdout}");
  assert!(stdout.contains("gate: clippy"), "{stdout}");
  assert!(!stdout.contains("gate: docs"), "{stdout}");
  assert!(stderr.contains("gate: fmt failed"), "{stderr}");
  assert!(stderr.contains("gate: clippy failed"), "{stderr}");
  assert!(stderr.contains("gate: failed: fmt, clippy"), "{stderr}");
}

#[test]
fn build_steps_still_stop_at_the_first_failure() {
  let dir = tempfile::tempdir().unwrap();
  invalid_cargo_project(dir.path());
  let output = gate_process(dir.path(), &["docs", "bench"]);
  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
  assert!(stdout.contains("gate: docs"), "{stdout}");
  assert!(!stdout.contains("gate: bench"), "{stdout}");
  assert!(stderr.contains("gate: docs failed"), "{stderr}");
}

#[test]
fn missing_tools_fail_closed() {
  let dir = tempfile::tempdir().unwrap();
  let err = require(
    dir.path(),
    "cargo-nope",
    "cargo".as_ref(),
    &["nope-subcommand", "--version"],
  )
  .unwrap_err();
  assert!(
    err.starts_with(
      "cargo-nope is not installed: cargo install cargo-nope --locked (CI installs it with \
       taiki-e/install-action) — error: no such command"
    ),
    "{err}"
  );
  let err = require(
    dir.path(),
    "cargo-gone",
    "no-such-program-xtask".as_ref(),
    &[],
  )
  .unwrap_err();
  assert!(err.contains("No such file or directory"), "{err}");
  let err = status(dir.path(), "no-such-program-xtask".as_ref(), &[], &[]).unwrap_err();
  assert!(err.starts_with("cannot run no-such-program-xtask"), "{err}");
}

#[test]
fn workspace_checks_run_in_process_on_the_repository() {
  let repo = std::fs::canonicalize(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).unwrap();
  assert_eq!(
    run(&options(repo, &["reach", "unused-pub"])).unwrap(),
    Verdict::Clean
  );
}
