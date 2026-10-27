use super::{FileContext, FileDecision, Status, inspect_file};
use crate::shape::tests::{isolated_env, repo};
use crate::shape::{FileKind, ShapeError};

#[test]
fn per_path_fallback_leaves_missing_blobs_unreviewed_and_fails_without_a_base() {
  let dir = repo();
  let status = Status {
    kind: FileKind::Modified,
    old_path: None,
    path: "missing.rs".to_owned(),
  };
  let env = isolated_env();
  let context = |merge_base| FileContext {
    env: &env,
    root: dir.path(),
    range: "HEAD...HEAD",
    merge_base,
    generated: None,
  };
  assert!(matches!(
    inspect_file(&context("HEAD"), &status),
    Ok(FileDecision::Unreviewed(_))
  ));
  assert!(matches!(
    inspect_file(&context("missing-base"), &status),
    Err(ShapeError::Git(_))
  ));
}
