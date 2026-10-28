//! The `semantic-git` rule over real git repositories: what it denies, what it
//! only explains, and what it leaves alone.

use serde_json::{Map, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::registry::RegistryEvent;

use super::SEMANTIC_GIT;
use crate::git_support::{Repo, git};

fn repo(config: Option<&str>) -> Repo {
  Repo::new(".claude", config)
}

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

fn decide(repo: &Repo, command: &str) -> Decision {
  let tool = Tool {
    call_id: text("c"),
    name: text("Bash"),
    input: json!({ "command": command }).as_object().cloned().unwrap(),
  };
  let event = NormalizedEvent::ShellPre {
    session: repo.session("s"),
    tool,
    command: text(command),
  };
  let raw = Map::new();
  let ctx = repo.context(Host::Claude, &raw);
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
    "git commit -m 'fix(x): y' --trailer 'Intent=z'",
    "git status && git log --oneline",
    "git checkout main && git branch",
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
  let renamed = denial(&decide(&repo, "git branch -m feature/x"));
  assert!(
    renamed.contains("`feature` is not a commit type"),
    "{renamed}"
  );
  let worktree = denial(&decide(&repo, "git worktree add -b Bad ../wt"));
  assert!(worktree.contains("Branch `Bad`:"), "{worktree}");
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
  let bad = denial(&decide(&repo, "git commit -F bad.txt"));
  assert!(bad.contains("the subject `fix stuff` is not"), "{bad}");
  std::fs::create_dir_all(repo.root.join("sub")).unwrap();
  std::fs::write(repo.root.join("sub/msg.txt"), "fix sub\n").unwrap();
  let sub = denial(&decide(&repo, "git -C sub commit -F msg.txt"));
  assert!(sub.contains("the subject `fix sub` is not"), "{sub}");
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

#[test]
fn a_dynamic_directory_or_branch_is_explained_and_a_failure_still_blocks() {
  let repo = repo(None);
  let dynamic = advice(&decide(&repo, "git -C \"$D\" commit -m 'fix stuff'"));
  assert!(
    dynamic.contains("the commit message, as its `-C` directory expands at run time"),
    "{dynamic}"
  );
  let both = denial(&decide(
    &repo,
    "git commit -m 'fix stuff'; git checkout -b \"$B\"",
  ));
  assert!(both.contains("the subject `fix stuff` is not"), "{both}");
}

#[test]
fn the_base_branch_is_exempt_even_where_it_exists_only_on_origin() {
  let repo = repo(None);
  git(
    &repo.root,
    &["update-ref", "refs/remotes/origin/trunk", "main"],
  );
  git(
    &repo.root,
    &[
      "symbolic-ref",
      "refs/remotes/origin/HEAD",
      "refs/remotes/origin/trunk",
    ],
  );
  assert_eq!(
    decide(&repo, "git checkout -b trunk origin/trunk"),
    Decision::Allow
  );
}

#[test]
fn config_values_that_cannot_be_read_are_reported_with_the_decision() {
  let repo = repo(Some(r#"{"version":1,"gitBetter":{"mode":"of"}}"#));
  let note = advice(&decide(&repo, "git commit -m 'fix(x): y' -m 'Intent: z'"));
  assert_eq!(
    note,
    "git-better config: gitBetter.mode is not one of block|advise|off; ignored"
  );
  let reason = denial(&decide(&repo, "git commit -m 'fix stuff'"));
  assert!(
    reason.ends_with("\ngit-better config: gitBetter.mode is not one of block|advise|off; ignored"),
    "{reason}"
  );
}
