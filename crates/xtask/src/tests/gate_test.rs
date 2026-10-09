use std::path::PathBuf;

use super::{STEPS, require, run, status};
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
      "clippy",
      "guardrails",
      "layers",
      "reach",
      "deny",
      "machete",
      "unused-pub",
      "hooks",
      "workflows",
      "docs-cli",
      "cli-compat",
      "jscpd",
      "rust-quality",
      "tests",
      "coverage",
      "docs",
      "context-budget",
      "portable-core",
      "workspace",
      "final-removal",
      "gate-coverage",
      "packaging",
      "ci-paths",
      "dist",
      "bench",
      "pack",
    ]
  );
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
fn dist_step_checks_an_empty_tree_without_a_mode_word() {
  let dir = tempfile::tempdir().unwrap();
  let verdict = run(&options(dir.path().to_path_buf(), &["dist"])).unwrap();
  assert_eq!(verdict, Verdict::Clean);
}

#[test]
fn bench_step_runs_the_deterministic_tier() {
  let dir = tempfile::tempdir().unwrap();
  let err = run(&options(dir.path().to_path_buf(), &["bench"])).unwrap_err();
  assert!(err.contains("queries file not found"), "{err}");
}

#[test]
fn a_failing_step_stops_the_gate() {
  let dir = tempfile::tempdir().unwrap();
  let verdict = run(&options(dir.path().to_path_buf(), &["fmt", "clippy"])).unwrap();
  assert_eq!(verdict, Verdict::Findings);
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
