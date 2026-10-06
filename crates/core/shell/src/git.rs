//! Git operations in a command line (`shell-git.ts`): the subcommand past git's
//! global options, the cumulative `-C` chain and refspec destination of a push,
//! and the static `-m` messages of a commit.

use crate::analysis::{ShellAnalysis, ShellCommand, Tristate, Word};
use crate::argv::basename;
use crate::options::{OptionSpec, Value, parse_args};

/// One `git` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInvocation<'a> {
  /// The command that runs it.
  pub command: &'a ShellCommand,
  /// The subcommand, or `None` when a dynamic word stands where it would be.
  pub subcommand: Option<&'a str>,
  /// Words after the subcommand.
  pub args: &'a [Word],
  /// Every `-C` value in order; git applies each relative to the previous one.
  pub c_chain: Vec<Option<&'a str>>,
}

const GLOBALS: OptionSpec = OptionSpec {
  value_short: "Cc",
  rest_short: "",
  value_long: "git-dir work-tree namespace super-prefix config-env attr-source",
  numeric: false,
  stop_at_operand: true,
  plus: false,
};

/// The git invocation `command` runs, or `None` when it is not git, malformed,
/// or has no subcommand.
pub fn git_invocation(command: &ShellCommand) -> Option<GitInvocation<'_>> {
  let name = command.argv.first()?.as_deref()?;
  if basename(name) != "git" {
    return None;
  }
  let globals = parse_args(&command.argv, 1, &GLOBALS);
  let subcommand = command.argv.get(globals.next)?;
  if globals.missing_value {
    return None;
  }
  let c_chain = globals.values("C").into_iter().map(Value::text).collect();
  Some(GitInvocation {
    command,
    subcommand: subcommand.as_deref(),
    args: command.argv.get(globals.next + 1..).unwrap_or_default(),
    c_chain,
  })
}

/// Whether the line runs `git <sub>`: `Unknown` when it cannot be ruled out
/// (the analysis is unknown, a command name is dynamic, or a git subcommand is).
pub fn runs_git_subcommand(analysis: &ShellAnalysis, sub: &str) -> Tristate {
  let mut unknown = analysis.unknown;
  for command in &analysis.commands {
    if matches!(command.argv.first(), Some(None)) {
      unknown = true;
      continue;
    }
    match git_invocation(command).map(|git| git.subcommand) {
      Some(Some(found)) if found == sub => return Tristate::Yes,
      Some(None) => unknown = true,
      _ => {}
    }
  }
  if unknown {
    Tristate::Unknown
  } else {
    Tristate::No
  }
}

/// A push's refspec: TypeScript's `undefined`, `null` or a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refspec<'a> {
  /// No refspec was given.
  Absent,
  /// The refspec is dynamic.
  Dynamic,
  /// A static refspec.
  Static(&'a str),
}

/// One `git push`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPush<'a> {
  /// The invocation, with its `-C` chain.
  pub invocation: GitInvocation<'a>,
  /// The refspec (second positional after `push`).
  pub refspec: Refspec<'a>,
  /// The branch the refspec pushes to, when it names exactly one.
  pub destination: Option<String>,
}

const PUSH_OPTIONS: OptionSpec = OptionSpec {
  value_short: "o",
  rest_short: "",
  value_long: "push-option receive-pack exec repo",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

/// `HEAD:x`, `+HEAD:refs/heads/x`, `src:x` and bare `x` name `x`; a delete,
/// bare `HEAD` or a wildcard names none.
fn destination_of(refspec: Refspec<'_>) -> Option<String> {
  let Refspec::Static(spec) = refspec else {
    return None;
  };
  let spec = spec.strip_prefix('+').unwrap_or(spec);
  if spec.starts_with(':') || spec == "HEAD" {
    return None;
  }
  let dst = spec.split_once(':').map_or(spec, |(_, dst)| dst);
  let dst = dst.strip_prefix("refs/heads/").unwrap_or(dst);
  (!dst.is_empty() && !dst.contains('*')).then(|| dst.to_owned())
}

/// Every `git push` in the line, in execution order.
pub fn push_targets(analysis: &ShellAnalysis) -> Vec<GitPush<'_>> {
  let mut pushes = Vec::new();
  for command in &analysis.commands {
    let Some(invocation) = git_invocation(command) else {
      continue;
    };
    if invocation.subcommand != Some("push") {
      continue;
    }
    let parsed = parse_args(invocation.args, 0, &PUSH_OPTIONS);
    let refspec = match parsed
      .operand_at
      .get(1)
      .and_then(|at| invocation.args.get(*at))
    {
      None => Refspec::Absent,
      Some(None) => Refspec::Dynamic,
      Some(Some(spec)) => Refspec::Static(spec),
    };
    let destination = destination_of(refspec);
    pushes.push(GitPush {
      invocation,
      refspec,
      destination,
    });
  }
  pushes
}

const COMMIT_OPTIONS: OptionSpec = OptionSpec {
  value_short: "mFCct",
  rest_short: "Su",
  value_long: "message file reuse-message reedit-message template author date cleanup fixup \
               squash trailer pathspec-from-file",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

/// The `-m`/`--message` values of a commit, in order; `None` for a dynamic one.
pub fn commit_messages(invocation: &GitInvocation<'_>) -> Vec<Option<String>> {
  if invocation.subcommand != Some("commit") {
    return Vec::new();
  }
  let parsed = parse_args(invocation.args, 0, &COMMIT_OPTIONS);
  let values = parsed.values("m message");
  values
    .into_iter()
    .map(|value| value.text().map(str::to_owned))
    .collect()
}

#[cfg(test)]
#[path = "tests/git_test.rs"]
mod tests;
