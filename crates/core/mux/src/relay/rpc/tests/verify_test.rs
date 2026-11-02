use super::input;
use crate::{hc::remote::export_recover, relay::store::Submission};
use toolu_runtime::{
  env::Env,
  process::{Spec, run},
};

fn git(repo: &str, args: &[&str]) -> String {
  let mut spec = Spec::new(["git", "-C", repo]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0, "{}", output.stderr);
  output.stdout.trim().to_owned()
}

#[test]
fn verifies_bundle_commit_ref_and_retry_before_acceptance() {
  let source = tempfile::tempdir().unwrap();
  let repo = source.path().to_str().unwrap();
  git(repo, &["init", "-q"]);
  git(repo, &["config", "user.name", "Test"]);
  git(repo, &["config", "user.email", "test@example.invalid"]);
  std::fs::write(source.path().join("file"), b"input").unwrap();
  git(repo, &["add", "file"]);
  git(repo, &["commit", "-qm", "input"]);
  let base = git(repo, &["rev-parse", "HEAD"]);
  let env = Env::process();
  let bundle = export_recover(&env, repo, "task", &base).unwrap();
  let wrong_ref = export_recover(&env, repo, "other", &base).unwrap();
  let request = Submission {
    key: "task".into(),
    origin: "unused".into(),
    base,
    brief: "work".into(),
    agent: "codex".into(),
    argv: vec!["codex".into()],
    machine: None,
    group: None,
  };
  let dest = tempfile::tempdir().unwrap();
  input(dest.path(), &env, &request, &bundle).unwrap();
  input(dest.path(), &env, &request, &bundle).unwrap();
  rejects_invalid_bundles(&env, &request, &bundle, &wrong_ref);
}

fn rejects_invalid_bundles(env: &Env, request: &Submission, bundle: &[u8], wrong_ref: &[u8]) {
  let wrong_base = Submission {
    base: "a".repeat(40),
    ..request.clone()
  };
  assert!(
    input(
      tempfile::tempdir().unwrap().path(),
      env,
      &wrong_base,
      bundle
    )
    .is_err()
  );
  assert!(input(tempfile::tempdir().unwrap().path(), env, request, wrong_ref).is_err());
  assert!(
    input(
      tempfile::tempdir().unwrap().path(),
      env,
      request,
      &bundle[..20]
    )
    .is_err()
  );
}
