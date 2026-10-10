//! Programs a shell command launches from `find`, `watch`, or package runners.

use std::path::Path;

use toolu_shell::analysis::ShellCommand;

fn name(command: &ShellCommand, at: usize) -> Option<&str> {
  Path::new(command.argv.get(at)?.as_deref()?)
    .file_name()
    .and_then(|name| name.to_str())
}

fn first_operand(command: &ShellCommand, from: usize) -> Option<usize> {
  command
    .argv
    .iter()
    .enumerate()
    .skip(from)
    .find(|(_, arg)| !arg.as_deref().unwrap_or("").starts_with('-'))
    .map(|(at, _)| at)
}

fn launched(command: &ShellCommand) -> Option<usize> {
  let verb = command.argv.get(1).and_then(Option::as_deref);
  match name(command, 0)? {
    "find" => command
      .argv
      .iter()
      .position(|arg| {
        matches!(
          arg.as_deref(),
          Some("-exec" | "-execdir" | "-ok" | "-okdir")
        )
      })
      .map(|at| at + 1),
    "npx" | "bunx" | "pnpx" | "watch" => first_operand(command, 1),
    "pnpm" | "yarn" if matches!(verb, Some("exec" | "dlx")) => first_operand(command, 2),
    "npm" if verb == Some("exec") => first_operand(command, 2),
    "bun" if verb == Some("x") => first_operand(command, 2),
    _ => None,
  }
}

/// Indexes of the command and one program it launches, if any.
pub(crate) fn program_indexes(command: &ShellCommand) -> Vec<usize> {
  match launched(command).filter(|at| *at < command.argv.len()) {
    Some(at) => vec![0, at],
    None => vec![0],
  }
}

/// Basename of a program at `at` in the command's argv.
pub(crate) fn program_at(command: &ShellCommand, at: usize) -> Option<&str> {
  name(command, at)
}

#[cfg(test)]
#[path = "tests/launched_test.rs"]
mod tests;
