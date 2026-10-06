//! The records an analysis produces (`shell-types.ts`). A consumer reads these,
//! never the parse tree: what runs, what it writes, which git operation, and
//! whether the exit status is observable are all pure functions over them.
//!
//! A `None` word is dynamic: its value depends on an expansion that only runs at
//! execution time. It is never guessed, so `$g push` has an unknown command name
//! rather than "not git".

/// A word as resolved statically: `None` when it expands at run time.
pub type Word = Option<String>;

/// The innermost context a command runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOrigin {
  /// Directly in the command line.
  Line,
  /// Inside `$(…)`, backticks, `<(…)`/`>(…)`, or an unquoted heredoc body.
  Substitution,
  /// Inside `bash -c STRING` (or sh/zsh/dash/ksh), or a static heredoc fed to a shell.
  Shell,
  /// Inside `eval ARGS`.
  Eval,
  /// Inside a function body: defined here, run only when called.
  Function,
}

impl CommandOrigin {
  /// The name TypeScript uses (`"line"`, `"substitution"`, …).
  pub fn as_str(self) -> &'static str {
    match self {
      CommandOrigin::Line => "line",
      CommandOrigin::Substitution => "substitution",
      CommandOrigin::Shell => "shell",
      CommandOrigin::Eval => "eval",
      CommandOrigin::Function => "function",
    }
  }
}

/// An answer that a dynamic command name or subcommand can leave open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tristate {
  /// Certainly.
  Yes,
  /// Certainly not.
  No,
  /// It cannot be ruled out.
  Unknown,
}

impl Tristate {
  /// The name TypeScript uses (`"yes"`, `"no"`, `"unknown"`).
  pub fn as_str(self) -> &'static str {
    match self {
      Tristate::Yes => "yes",
      Tristate::No => "no",
      Tristate::Unknown => "unknown",
    }
  }
}

/// A redirection operator, as bash spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectOperator {
  /// `>`
  Out,
  /// `>>`
  Append,
  /// `<`
  In,
  /// `<<`
  Heredoc,
  /// `<<-`
  HeredocStrip,
  /// `<<<`
  HereString,
  /// `<>`
  ReadWrite,
  /// `>&`
  DupOut,
  /// `<&`
  DupIn,
  /// `>|`
  Clobber,
  /// `&>`
  OutErr,
  /// `&>>`
  AppendOutErr,
}

impl RedirectOperator {
  /// Every operator, for tables and tests.
  pub const ALL: [RedirectOperator; 12] = [
    RedirectOperator::Out,
    RedirectOperator::Append,
    RedirectOperator::In,
    RedirectOperator::Heredoc,
    RedirectOperator::HeredocStrip,
    RedirectOperator::HereString,
    RedirectOperator::ReadWrite,
    RedirectOperator::DupOut,
    RedirectOperator::DupIn,
    RedirectOperator::Clobber,
    RedirectOperator::OutErr,
    RedirectOperator::AppendOutErr,
  ];

  /// The operator as written.
  pub fn as_str(self) -> &'static str {
    match self {
      RedirectOperator::Out => ">",
      RedirectOperator::Append => ">>",
      RedirectOperator::In => "<",
      RedirectOperator::Heredoc => "<<",
      RedirectOperator::HeredocStrip => "<<-",
      RedirectOperator::HereString => "<<<",
      RedirectOperator::ReadWrite => "<>",
      RedirectOperator::DupOut => ">&",
      RedirectOperator::DupIn => "<&",
      RedirectOperator::Clobber => ">|",
      RedirectOperator::OutErr => "&>",
      RedirectOperator::AppendOutErr => "&>>",
    }
  }

  /// The operator spelled `text`, if it is one.
  pub fn parse(text: &str) -> Option<RedirectOperator> {
    RedirectOperator::ALL
      .into_iter()
      .find(|operator| operator.as_str() == text)
  }
}

/// A heredoc body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heredoc {
  /// The body as the command reads it (tabs stripped for `<<-`), or `None` when it expands.
  pub content: Option<String>,
  /// The delimiter was quoted, so the body is data.
  pub quoted: bool,
}

/// One redirection of a command or a compound command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellRedirect {
  /// The operator.
  pub operator: RedirectOperator,
  /// The explicit descriptor (`2>`), or `None` when none was written.
  pub fd: Option<u32>,
  /// The target's static value; `None` when dynamic, a pathname pattern, or absent (a heredoc).
  pub target: Word,
  /// An unquoted pathname pattern target (`>.en[v]`): bash writes the one file it matches.
  pub pattern: Option<String>,
  /// The target with quotes removed and expansions as written (`$HOME/.env`); empty for a heredoc.
  pub text: String,
  /// A `<<`/`<<-` body.
  pub heredoc: Option<Heredoc>,
}

/// A command's place in its pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelinePosition {
  /// Zero-based position.
  pub index: usize,
  /// Commands in the pipeline.
  pub size: usize,
}

impl PipelinePosition {
  /// Outside any pipeline: `{ index: 0, size: 1 }`.
  pub const ALONE: PipelinePosition = PipelinePosition { index: 0, size: 1 };
}

/// One simple command the line runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand {
  /// Name and arguments as written, before unwrapping.
  pub words: Vec<Word>,
  /// The command that actually runs, after wrappers; xargs appends a trailing `None`.
  pub argv: Vec<Word>,
  /// Aligned with `argv`: the unexpanded pattern of a word that bash globs.
  pub patterns: Vec<Option<String>>,
  /// Aligned with `argv`: each word with quotes removed and expansions as written.
  pub texts: Vec<String>,
  /// Wrappers peeled off, outermost first (`sudo`, `timeout`, …).
  pub wrappers: Vec<String>,
  /// The command's own redirections.
  pub redirects: Vec<ShellRedirect>,
  /// Position in the enclosing pipeline.
  pub pipeline: PipelinePosition,
  /// An exit status of 0 for the whole line proves this command ran and exited 0.
  pub exit_proves: bool,
  /// Where it runs.
  pub origin: CommandOrigin,
  /// `bash -c` / `eval` nesting depth; 0 on the line itself.
  pub depth: usize,
  /// Source text of the command, in the script it was parsed from.
  pub text: String,
}

/// A problem met while reading the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellError {
  /// What went wrong.
  pub message: String,
  /// Byte offset in the script it was found in (the line, or an inner `-c` string).
  pub pos: usize,
  /// The script's origin.
  pub origin: CommandOrigin,
}

/// Everything known about one command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellAnalysis {
  /// The line.
  pub source: String,
  /// Every simple command, in execution order (substitutions before their command).
  pub commands: Vec<ShellCommand>,
  /// Redirects on compound commands: `{ …; } >f`, `( … ) >f`, loops, function definitions.
  pub compound_redirects: Vec<ShellRedirect>,
  /// Errors from the line and from every nested script.
  pub errors: Vec<ShellError>,
  /// Nothing reliable is known: the input was oversize, the parse was cancelled,
  /// nesting passed the limit, or it had errors and no command at all.
  pub unknown: bool,
}

#[cfg(test)]
#[path = "tests/analysis_test.rs"]
mod tests;
