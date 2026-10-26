//! How `deploy-guard` reads release tags in one Bash line: a `git tag` that
//! creates a tag on another commit, and a push or `gh` call that sends one.

use std::path::Path;

use toolu_shell::analysis::{ShellAnalysis, ShellCommand, Word};
use toolu_shell::git::git_invocation;

use crate::ci_diff::git;
use crate::deploy_line::{name, tag_like};

/// The `git tag` options that take the next word as their value.
const TAG_VALUES: [&str; 8] = [
  "-m",
  "-F",
  "-u",
  "--message",
  "--file",
  "--local-user",
  "--cleanup",
  "--trailer",
];

/// The `git tag` filters that imply list mode.
const LIST_FILTERS: [&str; 6] = [
  "-n",
  "--contains",
  "--no-contains",
  "--merged",
  "--no-merged",
  "--points-at",
];

/// The `git tag` that creates a tag on a commit other than `HEAD`.
pub(crate) fn retagging(analysis: &ShellAnalysis) -> Option<&str> {
  analysis
    .commands
    .iter()
    .find(|command| {
      git_invocation(command)
        .is_some_and(|git| git.subcommand == Some("tag") && tags_another_commit(git.args))
    })
    .map(|command| command.text.as_str())
}

/// Whether `git tag <args>` creates a tag whose target is given and is not `HEAD`.
fn tags_another_commit(args: &[Word]) -> bool {
  let mut operands = Vec::new();
  let mut words = args.iter();
  while let Some(word) = words.next() {
    match word.as_deref() {
      Some("-l" | "--list" | "-d" | "--delete" | "-v" | "--verify") => return false,
      Some(flag) if LIST_FILTERS.iter().any(|list| flag.starts_with(list)) => return false,
      Some(option) if TAG_VALUES.contains(&option) => {
        words.next();
      }
      Some(flag) if flag.starts_with('-') => {}
      operand => operands.push(operand),
    }
  }
  operands
    .get(1)
    .is_some_and(|target| *target != Some("HEAD"))
}

/// The `git push` or `gh` call that sends a tag for an existing commit other than
/// `HEAD`: a tag-like refspec source, or `gh --target`, that resolves elsewhere.
pub(crate) fn foreign_tag<'a>(root: &Path, analysis: &'a ShellAnalysis) -> Option<&'a str> {
  let commit = |rev: &str| {
    git(
      root,
      &["rev-parse", "-q", "--verify", &format!("{rev}^{{commit}}")],
    )
    .ok()
    .map(|sha| sha.trim().to_owned())
  };
  let head = commit("HEAD")?;
  analysis
    .commands
    .iter()
    .find(|command| {
      tag_sources(command)
        .into_iter()
        .any(|rev| commit(rev).is_some_and(|sha| sha != head))
    })
    .map(|command| command.text.as_str())
}

/// A push's static tag-like refspec sources, or the `gh` `--target` value.
fn tag_sources(command: &ShellCommand) -> Vec<&str> {
  let words: Vec<&str> = command.argv.iter().flatten().map(String::as_str).collect();
  match name(command) {
    Some("git") if git_invocation(command).is_some_and(|git| git.subcommand == Some("push")) => {
      words
        .iter()
        .filter(|word| !word.starts_with('-') && tag_like(word))
        .filter_map(|word| word.trim_start_matches('+').split(':').next())
        .filter(|source| !source.is_empty() && !source.contains('*') && *source != "tag")
        .collect()
    }
    Some("gh") => words
      .windows(2)
      .filter_map(|pair| match pair {
        ["--target", value] => Some(*value),
        [_, value] => value.strip_prefix("--target="),
        _ => None,
      })
      .collect(),
    _ => Vec::new(),
  }
}

#[cfg(test)]
#[path = "tests/deploy_tag_test.rs"]
mod tests;
