//! `bash_commands_decide` (`packages/toolu-core/src/gates/bash-commands.ts`),
//! ported over the Rust analysis.

use toolu_shell::analysis::{ShellAnalysis, ShellCommand};
use toolu_shell::rules::matches_rule;

/// A rule without a space is a substring of the command's source text; any
/// other rule is argv-aware.
fn matches(command: &ShellCommand, rule: &str) -> bool {
  if rule.contains(' ') {
    matches_rule(command, rule)
  } else {
    command.text.contains(rule)
  }
}

/// `unknown:<why>` when the line cannot be analyzed, else the first deny rule,
/// in order, that some command matches while no allow rule matches that same
/// command (`deny:<rule>`), else `allow`.
pub(crate) fn decide(analysis: &ShellAnalysis, allow: &[String], deny: &[String]) -> String {
  if analysis.unknown {
    let why = analysis
      .errors
      .first()
      .map_or("no command could be read", |error| error.message.as_str());
    return format!("unknown:{why}");
  }
  let denied_by = |rule: &String| {
    analysis
      .commands
      .iter()
      .any(|command| matches(command, rule) && !allow.iter().any(|ok| matches(command, ok)))
  };
  deny
    .iter()
    .find(|rule| denied_by(rule))
    .map_or_else(|| "allow".to_owned(), |rule| format!("deny:{rule}"))
}
