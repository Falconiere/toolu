use std::path::Path;

use toolu_runtime::env::Env;

use super::{ShapeError, git, git_found};

#[test]
fn git_command_returns_real_version_bytes() {
  let output = git(&Env::process(), Path::new("."), &["--version"]).expect("git");
  assert!(output.starts_with(b"git version "));
}

#[test]
fn git_failure_keeps_git_message() {
  let failed = git(&Env::process(), Path::new("."), &["no-such-subcommand"]);
  assert!(
    matches!(&failed, Err(ShapeError::Git(message)) if message.contains("is not a git command")),
    "{failed:?}"
  );
}

#[test]
fn git_found_separates_not_found_from_git_failure() {
  let env = Env::process();
  let query = |root: &str, name: &str| {
    git_found(
      &env,
      Path::new(root),
      &["rev-parse", "--verify", "--quiet", name],
    )
  };
  assert!(matches!(query(".", "HEAD^{commit}"), Ok(Some(_))));
  assert!(matches!(query(".", "absent-ref^{commit}"), Ok(None)));
  assert!(matches!(
    query("/nonexistent-shape-root", "HEAD^{commit}"),
    Err(ShapeError::Git(_))
  ));
}
