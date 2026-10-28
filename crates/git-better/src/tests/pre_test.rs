//! The `semantic-git` rule over real git repositories: what it denies, what it
//! only explains, and what it leaves alone.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, json};
use tempfile::TempDir;
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::RuleContext;

use super::SEMANTIC_GIT;

/// A repository on `main` with one commit and a feature branch `feat/1-x`.
struct Repo {
  _dir: TempDir,
  root: PathBuf,
  env: Env,
}

fn git(root: &Path, args: &[&str]) {
  let status = Command::new("git")
    .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-C"])
    .arg(root)
    .args(args)
    .status()
    .unwrap();
  assert!(status.success(), "git {args:?}");
}

fn repo(config: Option<&str>) -> Repo {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("repo");
  std::fs::create_dir_all(root.join(".claude")).unwrap();
  git(&root, &["init", "-q", "-b", "main"]);
  git(&root, &["commit", "-q", "--allow-empty", "-m", "init"]);
  git(&root, &["branch", "feat/1-x"]);
  if let Some(config) = config {
    std::fs::write(root.join(".claude/toolu.config.json"), config).unwrap();
  }
  let env = Env::from_pairs([
    ("PATH", std::env::var("PATH").unwrap_or_default()),
    ("HOME", dir.path().display().to_string()),
    (
      "TOOLU_CONFIG_DIR",
      dir.path().join("user").display().to_string(),
    ),
    ("TOOLU_PROJECT_DIR", root.display().to_string()),
    (
      "TOOLU_SETTINGS_DIR",
      dir.path().join("settings").display().to_string(),
    ),
  ]);
  Repo {
    _dir: dir,
    root,
    env,
  }
}

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

fn decide(repo: &Repo, command: &str) -> Decision {
  let session = Session {
    session_id: text("s"),
    cwd: text(&repo.root.display().to_string()),
    project_root: text(&repo.root.display().to_string()),
    worktree: text(&repo.root.display().to_string()),
  };
  let tool = Tool {
    call_id: text("c"),
    name: text("Bash"),
    input: json!({ "command": command }).as_object().cloned().unwrap(),
  };
  let event = NormalizedEvent::ShellPre {
    session,
    tool,
    command: text(command),
  };
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &repo.env,
    config_root: &repo.root,
    project_root: &repo.root,
    cwd: Some(&repo.root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  assert!(SEMANTIC_GIT.applies(&event, &ctx));
  SEMANTIC_GIT.run(&event, &ctx)
}

fn denial(decision: &Decision) -> String {
  let Decision::Deny { reason } = decision else {
    panic!("expected a deny, got {decision:?}");
  };
  reason.as_str().to_owned()
}

fn advice(decision: &Decision) -> String {
  let Decision::Advisory { message } = decision else {
    panic!("expected an advisory, got {decision:?}");
  };
  message.as_str().to_owned()
}

#[test]
fn the_rule_is_semantic_git_on_bash_and_shell() {
  assert_eq!(
    (
      SEMANTIC_GIT.spec(),
      SEMANTIC_GIT.name(),
      SEMANTIC_GIT.event()
    ),
    ("git-better@toolu", "semantic-git", RegistryEvent::ToolPre)
  );
}

#[test]
fn a_semantic_commit_or_branch_is_allowed() {
  let repo = repo(None);
  for command in [
    "git add -A && git commit -m \"$(cat <<'EOF'\nfeat(cli): add x\n\nIntent: agents need x\nEOF\n)\"",
    "git commit -m 'fix(x): y' --trailer 'Intent: z'",
    "git commit -F - <<'EOF'\nfix(x): y\n\nIntent: z\nEOF",
    "git checkout -b feat/2-add-x",
    "git switch -c fix/ENG-3-retry",
    "git worktree add -b docs/readme ../wt",
    "git status && git log --oneline",
    "echo 'git commit -m bad'",
  ] {
    assert_eq!(decide(&repo, command), Decision::Allow, "{command}");
  }
}

#[test]
fn a_message_that_is_not_semantic_is_denied_with_a_fix() {
  let repo = repo(None);
  let reason = denial(&decide(&repo, "git commit -am 'Fixed stuff.'"));
  assert!(
    reason.starts_with("git-better blocked this command"),
    "{reason}"
  );
  assert!(
    reason.contains("is not `type(scope): description`"),
    "{reason}"
  );
  assert!(reason.contains("add an `Intent:` trailer"), "{reason}");
  assert!(
    reason.contains("  <type>(<area>): <what changed>"),
    "{reason}"
  );
  let spaced = denial(&decide(
    &repo,
    "git commit -m 'feat(x)!: y' -m 'Intent: z\nBREAKING CHANGE: w'",
  ));
  assert!(spaced.contains("write `BREAKING-CHANGE:`"), "{spaced}");
}

#[test]
fn a_new_branch_that_is_not_semantic_is_denied_but_existing_ones_are_not() {
  let repo = repo(None);
  let reason = denial(&decide(&repo, "git checkout -b My_Branch"));
  assert!(reason.contains("Branch `My_Branch`:"), "{reason}");
  assert!(reason.contains("  <type>/my-branch"), "{reason}");
  denial(&decide(&repo, "git branch -m feature/x"));
  git(&repo.root, &["branch", "legacy"]);
  for command in [
    "git checkout -B legacy",
    "git branch -f main HEAD",
    "git switch -c release-please--branches--main",
  ] {
    assert_eq!(decide(&repo, command), Decision::Allow, "{command}");
  }
}

#[test]
fn a_message_file_is_read_from_the_repository() {
  let repo = repo(None);
  std::fs::write(repo.root.join("msg.txt"), "fix(x): y\n\nIntent: z\n").unwrap();
  assert_eq!(decide(&repo, "git commit -F msg.txt"), Decision::Allow);
  std::fs::write(repo.root.join("bad.txt"), "fix stuff\n").unwrap();
  denial(&decide(&repo, "git commit -F bad.txt"));
  std::fs::create_dir_all(repo.root.join("sub")).unwrap();
  std::fs::write(repo.root.join("sub/msg.txt"), "fix stuff\n").unwrap();
  denial(&decide(&repo, "git -C sub commit -F msg.txt"));
}

#[test]
fn what_cannot_be_read_before_it_runs_is_explained_not_blocked() {
  let repo = repo(None);
  for (command, why) in [
    (
      "git commit -m \"$MSG\"",
      "cannot check the commit message, as its `-m` value expands at run time",
    ),
    (
      "git commit -F later.txt",
      "the commit message, as its file `later.txt` cannot be read yet",
    ),
    (
      "printf x | git commit -F -",
      "the commit message, as it is piped into `-F -`",
    ),
    (
      "git commit",
      "the commit message, as it is written in an editor",
    ),
    (
      "git checkout -b \"$B\"",
      "the new branch name, as it expands at run time",
    ),
  ] {
    let message = advice(&decide(&repo, command));
    assert!(message.contains(why), "{command}: {message}");
    assert!(message.contains("toolu git-better check"), "{message}");
  }
}

#[test]
fn reused_derived_and_kept_messages_are_left_alone() {
  let repo = repo(None);
  for command in [
    "git commit --amend --no-edit",
    "git commit -C HEAD",
    "git commit --fixup HEAD",
    "git commit -m 'fixup! feat(x): y'",
  ] {
    assert_eq!(decide(&repo, command), Decision::Allow, "{command}");
  }
}

#[test]
fn wip_is_allowed_only_off_the_base_branch() {
  let repo = repo(None);
  denial(&decide(&repo, "git commit -m 'wip: half'"));
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  assert_eq!(decide(&repo, "git commit -m 'wip: half'"), Decision::Allow);
}

#[test]
fn advise_mode_explains_and_off_mode_checks_nothing() {
  let advise = repo(Some(r#"{"version":1,"gitBetter":{"mode":"advise"}}"#));
  let message = advice(&decide(&advise, "git commit -m 'fix stuff'"));
  assert!(message.contains("gitBetter.mode is advise"), "{message}");
  let off = repo(Some(r#"{"version":1,"gitBetter":{"mode":"off"}}"#));
  assert_eq!(decide(&off, "git commit -m 'fix stuff'"), Decision::Allow);
  assert_eq!(decide(&off, "git commit -m \"$MSG\""), Decision::Allow);
}
