//! `toolu`: the one binary behind every plugin (epic #402). Until #442 brings the
//! clap command tree, it answers `--version`, `--hook-protocol` and
//! `[<plugin>] hook <name> [--event <Event>] [--plugin-root <dir>]` (#412).
//!
//! Exit codes: 0 success, 2 blocked, 64 usage.

mod args;
mod hook;
mod output;
mod session_start;

use std::path::PathBuf;
use std::process::ExitCode;

use args::Command;
use toolu_protocol::HOOK_PROTOCOL;

/// This binary's version.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What a run prints and how it exits.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
  /// Exit status.
  pub(crate) code: u8,
  /// One line for standard output.
  pub(crate) stdout: Option<String>,
  /// One line for standard error.
  pub(crate) stderr: Option<String>,
}

/// What a run reads from its process, injected so tests can supply it.
pub(crate) struct Context<'a> {
  /// This executable's canonical path.
  pub(crate) exe: Option<PathBuf>,
  /// The hook payload, read only by hooks that need it.
  pub(crate) stdin: &'a dyn Fn() -> String,
}

/// Run `words` (argv after the program name).
pub(crate) fn run(words: &[String], context: &Context<'_>) -> Outcome {
  match args::parse(words) {
    Ok(Command::Version) => Outcome {
      stdout: Some(format!("toolu {VERSION}")),
      ..Outcome::default()
    },
    Ok(Command::HookProtocol) => Outcome {
      stdout: Some(HOOK_PROTOCOL.to_string()),
      ..Outcome::default()
    },
    Ok(Command::Hook(request)) => hook::run(&request, context),
    Err(message) => Outcome {
      code: 64,
      stdout: None,
      stderr: Some(format!("toolu: {message}\n{}", args::USAGE)),
    },
  }
}

fn main() -> ExitCode {
  let read = || toolu_protocol::stdin::read_stdin().unwrap_or_default();
  let context = Context {
    exe: toolu_runtime::invocation::current_exe(),
    stdin: &read,
  };
  let outcome = run(&toolu_runtime::invocation::args(), &context);
  output::emit(&outcome);
  ExitCode::from(outcome.code)
}

#[cfg(test)]
#[path = "tests/main_test.rs"]
mod tests;
