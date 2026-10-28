//! Branch names a git command creates, renames or copies to:
//! `checkout -b/-B/--orphan`, `switch -c/-C/--create/--force-create/--orphan`,
//! `branch <new>`, `branch -m/-M/-c/-C [<old>] <new>` and
//! `worktree add -b/-B`. Checkouts of an existing or remote branch name none.

use super::GitInvocation;
use crate::analysis::Word;
use crate::options::{OptionSpec, ParsedArgs, Value, parse_args};

/// How a command names its new branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchVerb {
  /// `git checkout -b/-B/--orphan`.
  Checkout,
  /// `git switch -c/-C/--create/--force-create/--orphan`.
  Switch,
  /// `git branch <new>`.
  Create,
  /// `git branch -m/-M/--move`.
  Rename,
  /// `git branch -c/-C/--copy`.
  Copy,
  /// `git worktree add -b/-B`.
  Worktree,
}

/// One branch name a command introduces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBranch {
  /// How it is introduced.
  pub verb: BranchVerb,
  /// The name; `None` when dynamic.
  pub name: Word,
}

const CHECKOUT_OPTIONS: OptionSpec = OptionSpec {
  value_short: "bB",
  rest_short: "",
  value_long: "orphan conflict pathspec-from-file",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

const SWITCH_OPTIONS: OptionSpec = OptionSpec {
  value_short: "cC",
  rest_short: "",
  value_long: "create force-create orphan conflict",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

const BRANCH_OPTIONS: OptionSpec = OptionSpec {
  value_short: "u",
  rest_short: "",
  value_long: "set-upstream-to contains no-contains merged no-merged points-at format sort",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

/// `git branch` options that list, delete or configure instead of creating.
const NOT_CREATING: &str = "l list a all r remotes d D delete v verbose show-current contains \
                            no-contains merged no-merged points-at format sort u set-upstream-to \
                            unset-upstream edit-description";

const WORKTREE_ADD_OPTIONS: OptionSpec = OptionSpec {
  value_short: "bB",
  rest_short: "",
  value_long: "reason",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

/// The values of `names`; a missing value (a malformed line) names nothing.
fn named(parsed: &ParsedArgs<'_>, names: &str, verb: BranchVerb) -> Vec<NewBranch> {
  let mut found = Vec::new();
  for value in parsed.values(names) {
    let name = match value {
      Value::Text(text) => Some(text.to_owned()),
      Value::Dynamic => None,
      Value::Absent => continue,
    };
    found.push(NewBranch { verb, name });
  }
  found
}

/// `git branch`: the first operand of a create, the last of a rename or copy.
fn branch(args: &[Word]) -> Vec<NewBranch> {
  let parsed = parse_args(args, 0, &BRANCH_OPTIONS);
  if parsed.has(NOT_CREATING) {
    return Vec::new();
  }
  let (verb, at) = if parsed.has("m M move") {
    (BranchVerb::Rename, parsed.operand_at.last())
  } else if parsed.has("c C copy") {
    (BranchVerb::Copy, parsed.operand_at.last())
  } else {
    (BranchVerb::Create, parsed.operand_at.first())
  };
  at.and_then(|at| args.get(*at))
    .map(|name| NewBranch {
      verb,
      name: name.clone(),
    })
    .into_iter()
    .collect()
}

/// Every branch name `invocation` introduces, in order.
pub fn new_branches(invocation: &GitInvocation<'_>) -> Vec<NewBranch> {
  let args = invocation.args;
  match invocation.subcommand {
    Some("checkout") => named(
      &parse_args(args, 0, &CHECKOUT_OPTIONS),
      "b B orphan",
      BranchVerb::Checkout,
    ),
    Some("switch") => named(
      &parse_args(args, 0, &SWITCH_OPTIONS),
      "c C create force-create orphan",
      BranchVerb::Switch,
    ),
    Some("branch") => branch(args),
    Some("worktree") if args.first().and_then(Option::as_deref) == Some("add") => named(
      &parse_args(args, 1, &WORKTREE_ADD_OPTIONS),
      "b B",
      BranchVerb::Worktree,
    ),
    _ => Vec::new(),
  }
}

#[cfg(test)]
#[path = "tests/branch_test.rs"]
mod tests;
