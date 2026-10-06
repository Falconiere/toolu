//! toolu core, `shell` layer: what a Bash/Shell command line runs, what it
//! writes, its git invocations, and whether its exit status is observable
//! (#416, the port of `@toolu/core/shell`). The line is parsed with
//! tree-sitter-bash, which recovers a partial tree from a syntax error, and
//! walked into flat records. See `docs/shell-analysis.md`.

pub mod analysis;
mod argv;
mod command;
mod fixup;
pub mod git;
mod heredoc;
mod options;
mod parse;
mod redirect;
pub mod rules;
mod walk;
mod words;
pub mod writes;

use analysis::{CommandOrigin, ShellAnalysis, ShellError};
use parse::Syntax;
pub use parse::{MAX_SHELL_INPUT, PARSE_BUDGET};
use walk::{Ctx, Walker};
pub use walk::{MAX_NESTING, MAX_RUN_DEPTH};

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "shell";

/// Nothing is known about `source`, for `message`.
fn unknown(source: &str, message: String, pos: usize) -> ShellAnalysis {
  let error = ShellError {
    message,
    pos,
    origin: CommandOrigin::Line,
  };
  ShellAnalysis {
    source: source.to_owned(),
    commands: Vec::new(),
    compound_redirects: Vec::new(),
    errors: vec![error],
    unknown: true,
  }
}

/// The analysis once the walk is done: a line with errors exits non-zero in
/// bash, so a zero status proves nothing about its commands.
fn finish(source: &str, walker: Walker) -> ShellAnalysis {
  let mut commands = walker.commands;
  let errored = !walker.errors.is_empty();
  if errored {
    for command in &mut commands {
      command.exit_proves = false;
    }
  }
  let unknown = errored && (commands.is_empty() || walker.overflow || walker.failed);
  ShellAnalysis {
    source: source.to_owned(),
    commands,
    compound_redirects: walker.compound_redirects,
    errors: walker.errors,
    unknown,
  }
}

/// Analyze one command line (`analyzeShell`). It never panics: input over
/// `MAX_SHELL_INPUT`, a parse past `PARSE_BUDGET`, nesting past `MAX_NESTING`,
/// and errors with no command at all are `unknown`.
pub fn analyze(source: &str) -> ShellAnalysis {
  if source.len() > MAX_SHELL_INPUT {
    let length = parse::utf16_len(source);
    if length > MAX_SHELL_INPUT {
      let message = format!("oversize: {length} characters exceeds the {MAX_SHELL_INPUT} cap");
      return unknown(source, message, MAX_SHELL_INPUT);
    }
  }
  let syntax = match Syntax::new(PARSE_BUDGET) {
    Ok(syntax) => syntax,
    Err(failure) => return unknown(source, failure.message(), 0),
  };
  let mut walker = Walker::new(syntax);
  match walker.syntax.script(source) {
    Ok((tree, text)) => walker.walk_script(&tree, Ctx::line(&text)),
    Err(failure) => walker.fail(&failure, CommandOrigin::Line),
  }
  finish(source, walker)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
