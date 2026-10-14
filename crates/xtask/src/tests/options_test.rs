use std::path::PathBuf;

use super::Options;

fn words(args: &[&str]) -> Vec<String> {
  args.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn flags_and_files_are_parsed() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().to_str().unwrap();
  let options = Options::parse(&words(&[
    "--root", root, "--base", "main", "--title", "feat: x", "--only", "fmt", "--only", "clippy",
    "cov.json",
  ]))
  .unwrap();
  assert_eq!(options.root, std::fs::canonicalize(root).unwrap());
  assert_eq!(options.base.as_deref(), Some("main"));
  assert_eq!(options.title.as_deref(), Some("feat: x"));
  assert_eq!(options.only, ["fmt", "clippy"]);
  assert_eq!(options.files, [PathBuf::from("cov.json")]);
}

#[test]
fn launcher_options_are_parsed() {
  let options = Options::parse(&words(&[
    "--timeout",
    "30",
    "--bin",
    "/usr/local/bin/toolu",
    "--home",
    "/h",
    "toolu",
  ]))
  .unwrap();
  assert_eq!(options.timeout.as_deref(), Some("30"));
  assert_eq!(options.bin, Some(PathBuf::from("/usr/local/bin/toolu")));
  assert_eq!(options.home, Some(PathBuf::from("/h")));
  assert_eq!(options.files, [PathBuf::from("toolu")]);
}

#[test]
fn the_default_root_is_this_repository() {
  let options = Options::parse(&[]).unwrap();
  assert!(options.root.join("crates/xtask/Cargo.toml").is_file());
}

#[test]
fn bad_options_are_errors() {
  assert_eq!(
    Options::parse(&words(&["--only"])).unwrap_err(),
    "--only needs a value"
  );
  assert_eq!(
    Options::parse(&words(&["--nope", "x"])).unwrap_err(),
    "unknown option --nope"
  );
  let err = Options::parse(&words(&["--root", "/no/such/dir"])).unwrap_err();
  assert!(err.starts_with("cannot resolve root /no/such/dir"), "{err}");
}

#[test]
fn measure_takes_out_and_every_word_after_the_separator() {
  let options = Options::parse(&words(&["--out", "r.json", "--", "sh", "-c", "--root x"])).unwrap();
  assert_eq!(options.out, Some(PathBuf::from("r.json")));
  assert_eq!(options.command, ["sh", "-c", "--root x"]);
  assert!(options.for_task("measure").is_ok());
}

#[test]
fn other_tasks_refuse_measure_bench_and_smoke_options() {
  for (args, task, err) in [
    (
      &["--out", "r.json"][..],
      "guardrails",
      "only measure and the benches take --out",
    ),
    (
      &["--", "true"][..],
      "bench-hooks",
      "only measure takes a command after --",
    ),
    (
      &["--assert"][..],
      "gate",
      "only the benches take --assert and --runs",
    ),
    (
      &["--runs", "3"][..],
      "measure",
      "only the benches take --assert and --runs",
    ),
    (
      &["--host", "claude"][..],
      "bench-hooks",
      "only host-smoke takes --host",
    ),
  ] {
    let options = Options::parse(&words(args)).unwrap();
    assert_eq!(options.for_task(task).unwrap_err(), err, "{args:?}");
  }
  let bench = Options::parse(&words(&["--assert", "--runs", "3", "--out", "r.json"])).unwrap();
  assert!(bench.assert);
  assert!(bench.for_task("bench-hooks").is_ok());
  let smoke = Options::parse(&words(&["--host", "codex"])).unwrap();
  assert_eq!(smoke.host.as_deref(), Some("codex"));
  assert!(smoke.for_task("host-smoke").is_ok());
}

#[test]
fn the_separator_is_never_an_option_value() {
  assert_eq!(
    Options::parse(&words(&["--out", "--", "true"])).unwrap_err(),
    "--out needs a value"
  );
}

#[test]
fn check_is_a_flag_without_a_value() {
  let options = Options::parse(&words(&["--check", "--bin", "/t/toolu"])).unwrap();
  assert!(options.check);
  assert_eq!(options.bin, Some(PathBuf::from("/t/toolu")));
  assert!(!Options::parse(&words(&[])).unwrap().check);
}
