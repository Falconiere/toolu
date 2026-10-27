use super::*;
use crate::docs_review::tests::fixture;
use toolu_shell::analyze;

#[test]
fn only_an_explicit_single_head_source_is_accepted() {
  let (dir, roots) = fixture();
  let repo = dir.path();
  for cmd in [
    "git push origin HEAD:feature",
    "git push -u origin feature",
    "git -C . push --force-with-lease origin HEAD:feature",
  ] {
    assert_eq!(
      reviewed_head(&analyze(cmd), repo, roots.env()),
      Ok(()),
      "{cmd}"
    );
  }
  for cmd in [
    "git push",
    "git push origin",
    "git push origin main",
    "git push origin HEAD:feature main:other",
    "git push --mirror origin HEAD:feature",
    "git push --all origin",
    "git push origin $REF",
    "git -c core.worktree=/tmp push origin HEAD:feature",
    "git push origin HEAD:feature && git status",
    "env GIT_DIR=/tmp git push origin HEAD:feature",
    "GIT_DIR=/tmp git push origin HEAD:feature",
    "git -C \"$dir\" push origin HEAD:feature",
    "git push --follow-tags origin HEAD:feature",
    "git push origin HEAD:refs/tags/release",
  ] {
    assert_eq!(
      reviewed_head(&analyze(cmd), repo, roots.env()),
      Err(SHAPE.to_owned()),
      "{cmd}"
    );
  }
}

#[test]
fn configured_follow_tags_disallows_even_an_explicit_head_push() {
  use std::process::Command;
  let (dir, roots) = fixture();
  let repo = dir.path();
  let status = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(["config", "push.followTags", "true"])
    .status()
    .unwrap();
  assert!(status.success());
  let error = reviewed_head(&analyze("git push origin HEAD:feature"), repo, roots.env())
    .expect_err("tags would be pushed");
  assert!(error.contains("add --no-follow-tags"), "{error}");
  assert_eq!(
    reviewed_head(
      &analyze("git push --no-follow-tags origin HEAD:feature"),
      repo,
      roots.env()
    ),
    Ok(())
  );
}
