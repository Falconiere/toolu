//! `toolu`: the one binary behind every plugin (epic #402). Each plugin crate
//! contributes a namespace to its clap tree (#442), and hooks take a fast path
//! that never builds the tree (#410's budget, `fast`).
//!
//! Exit codes: 0 success, 1 failure, 2 blocked, 64 usage, 69 unavailable,
//! 75 temporary failure (`toolu_protocol::exit::Exit`).

mod clap_error;
mod commands;
mod dispatch;
mod export;
mod fast;
mod hook;
mod links;
mod output;
mod registry;
mod session_start;
mod tree;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Command;
use fast::Fast;
use toolu_protocol::HOOK_PROTOCOL;
use toolu_runtime::cli::Outcome;

/// This binary's version.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What a run reads from its process, injected so tests can supply it.
pub(crate) struct Context<'a> {
  /// This executable's canonical path, resolved by `hook` runs only, so the
  /// per-hook `--version` and `--hook-protocol` probes make no extra syscall.
  pub(crate) exe: &'a dyn Fn() -> Option<PathBuf>,
  /// The hook payload, read only by hooks that need it.
  pub(crate) stdin: &'a dyn Fn() -> std::io::Result<String>,
}

/// Run `words` (argv after the program name). `tree` builds the clap tree; the
/// fast path never calls it.
pub(crate) fn run(words: &[String], context: &Context<'_>, tree: &dyn Fn() -> Command) -> Outcome {
  match fast::parse(words) {
    Some(Fast::HookProtocol) => Outcome::data(HOOK_PROTOCOL.to_string()),
    Some(Fast::Hook(request)) => hook::run(&request, context),
    None => dispatch::run(words, context, tree),
  }
}

fn main() -> ExitCode {
  let context = Context {
    exe: &toolu_runtime::invocation::current_exe,
    stdin: &toolu_protocol::stdin::read_stdin,
  };
  let outcome = run(&toolu_runtime::invocation::args(), &context, &tree::command);
  output::emit(&outcome);
  ExitCode::from(outcome.exit.code())
}

#[cfg(test)]
#[path = "tests/main_test.rs"]
mod tests;
