//! `shell-git.test.ts`: the subcommand past value-taking global options, a
//! tristate that never turns a dynamic name into "not a push", the `-C` chain
//! and refspec destination of a push, and the static messages of a commit.

use super::{Refspec, commit_messages, git_invocation, push_targets, runs_git_subcommand};
use crate::analysis::Tristate;
use crate::analyze;

fn push(source: &str) -> Tristate {
  runs_git_subcommand(&analyze(source), "push")
}

#[test]
fn a_push_behind_global_options_a_path_or_a_continuation_is_a_push() {
  for source in [
    "git push",
    "/usr/bin/git push",
    "git --git-dir .git push",
    "git --work-tree=. push",
    "git --attr-source HEAD push",
    "git -c push.default=simple push",
    "git -C a -C b push",
    "git --no-pager -p push",
    "git \\\n  push",
  ] {
    assert_eq!(push(source), Tristate::Yes, "{source}");
  }
}

#[test]
fn words_that_only_mention_push_are_not_a_push() {
  for source in [
    "git commit -m \"push\"",
    "echo git push",
    "echo \"no git push rules\"",
    "git pushup",
    "gitpush",
    "git log --grep push",
    "cat <<EOF\ngit push\nEOF",
  ] {
    assert_eq!(push(source), Tristate::No, "{source}");
  }
}

#[test]
fn a_dynamic_name_or_subcommand_is_unknown() {
  for source in [
    "$g push",
    "git $(echo push)",
    "gi$(echo t) push",
    "git $SUB",
    "sudo $CMD",
    "/usr/bin/g[i]t push",
    "git pu?h",
    ")",
  ] {
    assert_eq!(push(source), Tristate::Unknown, "{source}");
  }
}

#[test]
fn a_malformed_or_subcommand_less_git_runs_nothing() {
  for source in ["git -C", "git -c", "git -C dir", "git --version", "git"] {
    let analysis = analyze(source);
    assert_eq!(git_invocation(&analysis.commands[0]), None, "{source}");
    assert_eq!(push(source), Tristate::No, "{source}");
  }
}

#[test]
fn the_commit_subcommand_is_exact() {
  assert_eq!(
    runs_git_subcommand(&analyze("git commit-tree abc"), "commit"),
    Tristate::No
  );
  let in_tree = analyze("git -C /tmp/wt commit -m x");
  assert_eq!(runs_git_subcommand(&in_tree, "commit"), Tristate::Yes);
}

#[test]
fn a_push_reports_its_c_chain_and_refspec() {
  let analysis = analyze("git -C \"/tmp/my wt\" -C inner push origin +HEAD:refs/heads/feat/x");
  let targets = push_targets(&analysis);
  assert_eq!(
    targets[0].invocation.c_chain,
    [Some("/tmp/my wt"), Some("inner")]
  );
  assert_eq!(
    targets[0].refspec,
    Refspec::Static("+HEAD:refs/heads/feat/x")
  );
  assert_eq!(targets[0].destination.as_deref(), Some("feat/x"));
  let two = analyze("git -C a log --grep push; git -C b push");
  assert_eq!(push_targets(&two)[0].invocation.c_chain, [Some("b")]);
}

#[test]
fn the_destination_follows_the_push_target_branch_contract() {
  let cases: [(&str, Option<&str>); 13] = [
    ("git push origin HEAD:feat/x", Some("feat/x")),
    ("git push -u origin feat/x", Some("feat/x")),
    ("git push origin abc123:feat/x", Some("feat/x")),
    ("git push -o ci.skip origin HEAD:feat/x", Some("feat/x")),
    (
      "git push --push-option=ci.skip origin HEAD:feat/x",
      Some("feat/x"),
    ),
    ("git push --repo origin origin HEAD:feat/x", Some("feat/x")),
    ("git push origin HEAD:feat/x 2>&1 | tee log", Some("feat/x")),
    ("git push", None),
    ("git push origin", None),
    ("git push origin HEAD", None),
    ("git push origin :feat/x", None),
    ("git push origin 'refs/heads/*:refs/heads/*'", None),
    ("git push origin $BRANCH", None),
  ];
  for (source, expected) in cases {
    let analysis = analyze(source);
    let destination = push_targets(&analysis)[0].destination.clone();
    assert_eq!(destination.as_deref(), expected, "{source}");
  }
}

#[test]
fn a_refspec_is_absent_dynamic_or_static() {
  let refspec = |source: &str| {
    let analysis = analyze(source);
    let found = push_targets(&analysis)[0].refspec;
    format!("{found:?}")
  };
  assert_eq!(refspec("git push origin"), "Absent");
  assert_eq!(refspec("git push origin $B"), "Dynamic");
  assert_eq!(refspec("git push origin main"), "Static(\"main\")");
}

fn messages(source: &str) -> Vec<Option<String>> {
  let analysis = analyze(source);
  let commit = analysis
    .commands
    .iter()
    .filter_map(git_invocation)
    .find(|git| git.subcommand == Some("commit"));
  commit.map(|git| commit_messages(&git)).unwrap_or_default()
}

fn some(values: &[&str]) -> Vec<Option<String>> {
  values
    .iter()
    .map(|value| Some((*value).to_owned()))
    .collect()
}

#[test]
fn commit_messages_are_read_in_every_form() {
  assert_eq!(
    messages("git commit -m \"$(cat <<'EOF'\nfeat: x\n\nbody\nEOF\n)\""),
    some(&["feat: x\n\nbody"])
  );
  assert_eq!(messages("git commit -am \"fix: y\""), some(&["fix: y"]));
  assert_eq!(messages("git commit -m'chore: z'"), some(&["chore: z"]));
  assert_eq!(
    messages("git commit --message=\"docs: w\""),
    some(&["docs: w"])
  );
  assert_eq!(
    messages("git commit --message docs: -m second"),
    some(&["docs:", "second"])
  );
  assert_eq!(messages("git commit -m \"$MSG\""), [None]);
  assert_eq!(
    messages("git commit -F msg.txt"),
    Vec::<Option<String>>::new()
  );
  assert_eq!(
    messages("git add -A && git commit -m \"feat: x\""),
    some(&["feat: x"])
  );
  let push = analyze("git push -m x");
  assert_eq!(
    commit_messages(&git_invocation(&push.commands[0]).unwrap()),
    Vec::<Option<String>>::new()
  );
}

#[test]
fn a_push_before_a_syntax_error_is_still_a_push() {
  let analysis = analyze("git push; echo \"unterminated");
  assert_ne!(analysis.errors, Vec::<crate::analysis::ShellError>::new());
  assert_eq!(runs_git_subcommand(&analysis, "push"), Tristate::Yes);
  assert_eq!(push("git push\necho $("), Tristate::Yes);
}
