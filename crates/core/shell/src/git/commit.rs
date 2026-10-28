//! Where a `git commit` takes its message from: `-m` values, `-F` files (`-`
//! reads a static heredoc or herestring), `--trailer` values, and the options
//! that reuse or derive a message instead of taking a new one.

use super::GitInvocation;
use crate::analysis::Word;
use crate::options::{OptionSpec, ParsedArgs, Value, parse_args};
use crate::redirect::stdin_script;

const COMMIT_OPTIONS: OptionSpec = OptionSpec {
  value_short: "mFCct",
  rest_short: "Su",
  value_long: "message file reuse-message reedit-message template author date cleanup fixup \
               squash trailer pathspec-from-file",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

/// Where a commit's message comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageOrigin {
  /// Written for this commit: `-m`, `-F`, `--trailer`, or the editor.
  #[default]
  Written,
  /// Taken from another commit: `-C`/`-c`/`--reuse-message`/`--reedit-message`.
  Reused,
  /// Derived by git from the target commit: `--fixup`/`--squash`.
  Derived,
  /// `--amend --no-edit` with no new text: HEAD's message is kept.
  Kept,
}

/// The message sources of one `git commit`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommitArgs {
  /// `-m`/`--message` values in order; `None` for a dynamic one. Git joins them
  /// as paragraphs.
  pub messages: Vec<Word>,
  /// `-F`/`--file` values in order; `None` for a dynamic one, `Some("-")` for stdin.
  pub files: Vec<Word>,
  /// The static heredoc or herestring on the command's stdin, which `-F -` reads.
  pub stdin: Option<String>,
  /// `--trailer` values in order; `None` for a dynamic one.
  pub trailers: Vec<Word>,
  /// Where the message comes from.
  pub origin: MessageOrigin,
  /// `--amend`.
  pub amend: bool,
}

fn words(parsed: &ParsedArgs<'_>, names: &str) -> Vec<Word> {
  parsed
    .values(names)
    .into_iter()
    .map(|value| match value {
      Value::Text(text) => Some(text.to_owned()),
      Value::Absent | Value::Dynamic => None,
    })
    .collect()
}

/// What a `git commit` reads its message from; `None` for any other subcommand.
pub fn commit_args(invocation: &GitInvocation<'_>) -> Option<CommitArgs> {
  if invocation.subcommand != Some("commit") {
    return None;
  }
  let parsed = parse_args(invocation.args, 0, &COMMIT_OPTIONS);
  let messages = words(&parsed, "m message");
  let files = words(&parsed, "F file");
  let amend = parsed.has("amend");
  let origin = if parsed.has("C c reuse-message reedit-message") {
    MessageOrigin::Reused
  } else if parsed.has("fixup squash") {
    MessageOrigin::Derived
  } else if amend && parsed.has("no-edit") && messages.is_empty() && files.is_empty() {
    MessageOrigin::Kept
  } else {
    MessageOrigin::Written
  };
  Some(CommitArgs {
    messages,
    files,
    stdin: stdin_script(&invocation.command.redirects),
    trailers: words(&parsed, "trailer"),
    origin,
    amend,
  })
}

/// The `-m`/`--message` values of a commit, in order; `None` for a dynamic one.
pub fn commit_messages(invocation: &GitInvocation<'_>) -> Vec<Word> {
  commit_args(invocation).map_or_else(Vec::new, |args| args.messages)
}

#[cfg(test)]
#[path = "tests/commit_test.rs"]
mod tests;
