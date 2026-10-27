use toolu_shell::analyze;

use super::{is_git_commit, is_git_push, possible_push};

#[test]
fn unknown_to_the_shell_layer_maps_to_false() {
  for command in ["$g push", "git $(echo push)"] {
    assert!(!is_git_push(&analyze(command)), "{command}");
  }
  assert!(!is_git_commit(&analyze("\"$GIT\" commit -m x")));
  assert!(is_git_push(&analyze("git -C repo push origin main")));
  assert!(is_git_commit(&analyze("echo a && git commit -m x")));
  assert!(!is_git_push(&analyze("git status")));
}

#[test]
fn a_heredoc_body_runs_nothing() {
  let lines = |parts: &[&str]| parts.join("\n");
  let cases = [
    (
      lines(&[
        "cat <<EOF > /tmp/x",
        "git push origin main",
        "EOF",
        "echo after",
      ]),
      false,
    ),
    (
      lines(&["cat <<-END", "\tgit push", "\tEND", "echo end"]),
      false,
    ),
    (lines(&["cat <<DOC", "git push", "DOC", "echo end"]), false),
    (lines(&["echo hello", "ls -la", "git push"]), true),
    (
      lines(&[
        "cat <<EOF | tee /tmp/x",
        "secret git push inside body",
        "EOF",
        "echo done",
      ]),
      false,
    ),
  ];
  for (command, push) in cases {
    assert_eq!(is_git_push(&analyze(&command)), push, "{command}");
  }
}

/// A repository with a pushing and a reading alias, and an env whose HOME
/// isolates the lookup from the developer's own aliases.
fn alias_repo() -> (tempfile::TempDir, toolu_runtime::env::Env) {
  let dir = tempfile::tempdir().unwrap();
  let repo = dir.path();
  for args in [
    &["init", "-q"][..],
    &["config", "alias.configured", "push"],
    &["config", "alias.lg", "log"],
  ] {
    let status = std::process::Command::new("git")
      .arg("-C")
      .arg(repo)
      .args(args)
      .env("HOME", repo)
      .status()
      .unwrap();
    assert!(status.success(), "{args:?}");
  }
  let env = toolu_runtime::env::Env::from_pairs([("HOME", repo.to_str().unwrap())]);
  (dir, env)
}

#[test]
fn hidden_and_plumbing_pushes_may_push() {
  let (dir, env) = alias_repo();
  let repo = dir.path();
  for command in [
    "git \"$sub\" log",
    "for c in status log; do git \"$c\"; done",
    "git $(printf 'p%s' ush) origin HEAD:feature",
    "git -c alias.p=push p origin HEAD:feature",
    "git --config-env=alias.p=P p origin HEAD:feature",
    "git config alias.p push && git p origin HEAD:feature",
    "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.p GIT_CONFIG_VALUE_0=push git p origin HEAD:feature",
    "CMD='git push origin main'; sh -c \"$CMD\"",
    "$git_command push origin main",
    "git send-pack origin HEAD:refs/heads/feature",
    "git subtree push --prefix=docs origin feature",
    "git configured origin HEAD:feature",
    "git commit -F - <<EOF; git push origin HEAD:feature\nbody\nEOF",
  ] {
    assert!(
      possible_push(&analyze(command), repo, &env).is_some(),
      "{command}"
    );
  }
  assert_eq!(
    possible_push(&analyze("git -C sub $(echo p) origin main"), repo, &env),
    Some(repo.join("sub"))
  );
}

#[test]
fn static_and_read_only_git_may_not_push() {
  let (dir, env) = alias_repo();
  let repo = dir.path();
  for command in [
    "git lg",
    "git push origin HEAD:feature",
    "git stash push -m wip",
    "git -C \"$WT\" log --oneline --grep push",
    "git -C \"$WT\" status && echo \"ready to push\"",
    "git -c \"user.email=$EMAIL\" commit -m x",
    "eval \"$(brew shellenv)\"",
    "$formatter file",
  ] {
    assert_eq!(
      possible_push(&analyze(command), repo, &env),
      None,
      "{command}"
    );
  }
}
