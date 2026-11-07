use toolu_shell::analyze;

use super::*;

#[test]
fn a_static_push_is_found_and_an_oversize_command_is_not() {
  assert!(is_git_push(&analyze(
    "git -C repo push origin HEAD:feature"
  )));
  assert!(!is_git_push(&analyze("echo git push")));
}

#[test]
fn changed_names_quote_non_ascii_paths_even_when_the_repo_disables_quotepath() {
  let dir = tempfile::tempdir().expect("tempdir");
  let root = dir.path();
  let path = std::env::var("PATH").expect("PATH");
  let home = root.display().to_string();
  let env = Env::from_pairs([
    ("PATH", path.as_str()),
    ("HOME", home.as_str()),
    ("GIT_CONFIG_NOSYSTEM", "1"),
  ]);
  let git = |args: &[&str]| assert!(git_at(root, args, &env).is_some(), "git {args:?}");
  git(&["init", "-q", "-b", "main"]);
  git(&["config", "user.email", "test@example.com"]);
  git(&["config", "user.name", "Test"]);
  git(&["config", "core.quotepath", "false"]);
  git(&["commit", "-q", "--allow-empty", "-m", "base"]);
  git(&["checkout", "-q", "-b", "feature"]);
  std::fs::write(root.join("café.rs"), "fn main() {}\n").expect("write");
  git(&["add", "-A"]);
  git(&["commit", "-q", "-m", "feature"]);
  // The push-review writer and the verdict gate both record this spelling.
  let changed = changed_names(root, "main", &env);
  assert_eq!(changed.lines().collect::<Vec<_>>(), [r#""caf\303\251.rs""#]);
}
