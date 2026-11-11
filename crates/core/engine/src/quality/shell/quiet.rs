//! Conservative read-only classification for shell post quality checks.

use std::path::Path;

use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analysis::ShellCommand;
use toolu_shell::git::git_invocation;
use toolu_shell::writes::write_targets;

use super::super::QualityBinding;

/// Whether a path belongs to an admitted quality rule.
pub(super) fn has_extension(path: &str, bindings: &[QualityBinding<'_>], active: &[usize]) -> bool {
  let Some(extension) = Path::new(path)
    .extension()
    .and_then(|extension| extension.to_str())
  else {
    return false;
  };
  active.iter().any(|index| {
    bindings
      .get(*index)
      .is_some_and(|binding| binding.rule.extensions().contains(&extension))
  })
}

fn git_read_only(command: &ShellCommand) -> bool {
  git_invocation(command).is_some_and(|git| {
    let safe = matches!(
      git.subcommand,
      Some(
        "status"
          | "diff"
          | "log"
          | "show"
          | "rev-parse"
          | "ls-files"
          | "grep"
          | "blame"
          | "cat-file"
          | "describe"
      )
    );
    safe
      && git.args.iter().all(|arg| {
        arg
          .as_deref()
          .is_some_and(|arg| !arg.starts_with("--output") && arg != "-o")
      })
  })
}

fn cargo_read_only(command: &ShellCommand) -> bool {
  let at = if command
    .argv
    .get(1)
    .and_then(Option::as_deref)
    .is_some_and(|arg| arg.starts_with('+'))
  {
    2
  } else {
    1
  };
  let sub = command.argv.get(at).and_then(Option::as_deref);
  matches!(sub, Some("build" | "check" | "test" | "nextest" | "clippy"))
    && command.argv.iter().all(|arg| {
      arg
        .as_deref()
        .is_some_and(|arg| arg != "--fix" && !arg.starts_with("--fix="))
    })
}

fn read_only(command: &ShellCommand) -> bool {
  if command.argv.iter().any(Option::is_none) {
    return false;
  }
  let name = command
    .argv
    .first()
    .and_then(Option::as_deref)
    .unwrap_or_default();
  match name
    .trim_end_matches('/')
    .rsplit('/')
    .next()
    .unwrap_or(name)
  {
    "cat" | "rg" | "ls" => true,
    "git" => git_read_only(command),
    "cargo" => cargo_read_only(command),
    _ => false,
  }
}

/// A known read-only line whose named targets cannot be quality source files.
pub(super) fn quiet(ctx: &RuleContext<'_>, bindings: &[QualityBinding<'_>]) -> bool {
  let analysis = crate::gates::command_analysis(ctx);
  if analysis.unknown || analysis.commands.is_empty() {
    return false;
  }
  let all: Vec<usize> = (0..bindings.len()).collect();
  analysis.commands.iter().all(read_only)
    && write_targets(&analysis).iter().all(|target| {
      target
        .path
        .as_deref()
        .is_some_and(|path| !has_extension(path, bindings, &all))
    })
}

#[cfg(test)]
#[path = "tests/quiet_test.rs"]
mod tests;
