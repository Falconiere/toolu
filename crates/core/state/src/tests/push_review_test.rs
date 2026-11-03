use std::fs;

use toolu_runtime::json::ordered::Ordered;

use super::prior_round;

#[test]
fn an_old_state_increments_only_for_the_same_diff() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("feature.json");
  let old = include_str!("../../../../cli/tests/fixtures/review_legacy.json");
  fs::write(&path, old).unwrap();
  let sha = Ordered::parse(old)
    .unwrap()
    .get("diff_sha")
    .cloned()
    .unwrap();
  let Ordered::String(sha) = sha else {
    panic!("fixture diff hash must be a string");
  };
  assert_eq!(prior_round(&path, &sha), 1);
  assert_eq!(prior_round(&path, "changed"), 0);
  fs::write(&path, "not json").unwrap();
  assert_eq!(prior_round(&path, &sha), 0);
}

#[test]
fn empty_committed_diff_can_record_a_blocker_but_not_an_approval() {
  use super::{Input, write};
  use toolu_runtime::cli::Ctx;
  use toolu_runtime::env::Env;

  let repo = tempfile::tempdir().unwrap();
  let state = tempfile::tempdir().unwrap();
  init_empty_repo(repo.path());
  let env = Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("PUSH_REVIEW_BASE", "main")
    .with("STATE_DIR", &state.path().to_string_lossy());
  let mut input = Input {
    findings_count: 0,
    reviewers: Ordered::parse("[]").unwrap(),
    findings: Ordered::parse("[]").unwrap(),
    repo: repo.path().to_str().unwrap(),
    branch: None,
    reviewed_files: None,
    expected_diff_sha: None,
  };
  let ctx = Ctx::default();
  assert!(write(&input, &ctx, &env).is_err());
  assert!(!state.path().join("feature.json").exists());
  input.findings_count = 1;
  let path = write(&input, &ctx, &env).unwrap();
  let saved = Ordered::parse(&fs::read_to_string(path).unwrap()).unwrap();
  assert_eq!(
    saved.get("findings_count"),
    Some(&Ordered::Number(1.into()))
  );
  assert_eq!(
    saved.get("reviewed_files"),
    Some(&Ordered::Array(Vec::new()))
  );
}

fn init_empty_repo(root: &std::path::Path) {
  use toolu_runtime::env::Env;
  use toolu_runtime::process::{Spec, run};

  for args in [
    vec!["init", "-q", "-b", "main"],
    vec![
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.com",
      "commit",
      "--allow-empty",
      "-qm",
      "base",
    ],
    vec!["switch", "-qc", "feature"],
  ] {
    let mut spec = Spec::new(["git"]);
    spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
    spec.cwd = Some(root.to_path_buf());
    spec.env = Some(
      Env::process()
        .with("GIT_CONFIG_NOSYSTEM", "1")
        .with("GIT_CONFIG_GLOBAL", "/dev/null"),
    );
    let out = run(&spec).unwrap();
    assert_eq!(out.exit_code, 0, "git {args:?}: {}", out.stderr);
  }
}
