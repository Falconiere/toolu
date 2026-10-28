//! The branch names checkout, switch, branch and worktree introduce, and the
//! forms that list, delete or reuse a branch instead.

use super::{BranchVerb, NewBranch, new_branches};
use crate::analyze;
use crate::git::git_invocation;

fn branches(source: &str) -> Vec<NewBranch> {
  let analysis = analyze(source);
  analysis
    .commands
    .iter()
    .filter_map(git_invocation)
    .flat_map(|git| new_branches(&git))
    .collect()
}

fn one(verb: BranchVerb, name: &str) -> Vec<NewBranch> {
  vec![NewBranch {
    verb,
    name: Some(name.to_owned()),
  }]
}

#[test]
fn checkout_and_switch_name_their_new_branch_in_every_form() {
  use BranchVerb::{Checkout, Switch};
  let cases = [
    ("git checkout -b feat/1-x", one(Checkout, "feat/1-x")),
    (
      "git checkout -B feat/x origin/main",
      one(Checkout, "feat/x"),
    ),
    ("git checkout -bfeat/x", one(Checkout, "feat/x")),
    (
      "git checkout --orphan docs/site",
      one(Checkout, "docs/site"),
    ),
    ("git -C wt checkout -q -b fix/y", one(Checkout, "fix/y")),
    ("git switch -c feat/x", one(Switch, "feat/x")),
    ("git switch -C feat/x main", one(Switch, "feat/x")),
    ("git switch --create feat/x", one(Switch, "feat/x")),
    ("git switch --create=feat/x", one(Switch, "feat/x")),
    ("git switch --force-create feat/x", one(Switch, "feat/x")),
    ("git switch --orphan feat/x", one(Switch, "feat/x")),
  ];
  for (source, expected) in cases {
    assert_eq!(branches(source), expected, "{source}");
  }
}

#[test]
fn branch_creates_renames_and_copies() {
  use BranchVerb::{Copy, Create, Rename};
  let cases = [
    ("git branch feat/x", one(Create, "feat/x")),
    ("git branch -f feat/x main", one(Create, "feat/x")),
    (
      "git branch --track feat/x origin/feat/x",
      one(Create, "feat/x"),
    ),
    ("git branch -m feat/new", one(Rename, "feat/new")),
    ("git branch -M old feat/new", one(Rename, "feat/new")),
    ("git branch --move old feat/new", one(Rename, "feat/new")),
    ("git branch -c old feat/copy", one(Copy, "feat/copy")),
    ("git branch -C feat/copy", one(Copy, "feat/copy")),
  ];
  for (source, expected) in cases {
    assert_eq!(branches(source), expected, "{source}");
  }
}

#[test]
fn worktree_add_names_only_an_explicit_branch() {
  assert_eq!(
    branches("git worktree add -b feat/x ../wt main"),
    one(BranchVerb::Worktree, "feat/x")
  );
  assert_eq!(
    branches("git worktree add --lock --reason why -B fix/y ../wt"),
    one(BranchVerb::Worktree, "fix/y")
  );
  assert_eq!(branches("git worktree add ../wt feat/x"), []);
  assert_eq!(branches("git worktree list"), []);
}

#[test]
fn listing_deleting_and_switching_to_existing_branches_name_nothing() {
  for source in [
    "git branch",
    "git branch -a",
    "git branch --list 'feat/*'",
    "git branch -d feat/x",
    "git branch -D feat/x",
    "git branch --delete feat/x",
    "git branch -vv",
    "git branch --show-current",
    "git branch --contains abc",
    "git branch --merged main",
    "git branch -u origin/x feat/x",
    "git branch --set-upstream-to=origin/x",
    "git branch --edit-description",
    "git branch -r",
    "git branch --remotes --list 'origin/*'",
    "git branch --unset-upstream",
    "git branch --format '%(refname)' --sort -committerdate",
    "git branch -m",
    "git checkout feat/x",
    "git checkout -- file.rs",
    "git switch main",
    "git switch -",
    "git checkout -b",
    "git commit -m 'git checkout -b x'",
  ] {
    assert_eq!(branches(source), [], "{source}");
  }
}

#[test]
fn a_dynamic_name_is_reported_as_unknown() {
  assert_eq!(
    branches("git checkout -b \"$BRANCH\""),
    [NewBranch {
      verb: BranchVerb::Checkout,
      name: None
    }]
  );
  assert_eq!(
    branches("git branch $B"),
    [NewBranch {
      verb: BranchVerb::Create,
      name: None
    }]
  );
}

#[test]
fn several_names_in_one_command_are_each_reported() {
  assert_eq!(
    branches("git checkout -b feat/a -B feat/b"),
    [
      NewBranch {
        verb: BranchVerb::Checkout,
        name: Some("feat/a".to_owned())
      },
      NewBranch {
        verb: BranchVerb::Checkout,
        name: Some("feat/b".to_owned())
      },
    ]
  );
}
