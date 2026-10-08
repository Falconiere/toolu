//! Conventional Commit prefix check and the before-commit reminder.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::config::settings::{COMMIT_PREFIXES, read_list};
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analysis::ShellAnalysis;
use toolu_shell::git::{commit_messages, git_invocation};
use toolu_state::git::base_branch;

use super::{command_analysis, decided, gate_config, gate_settings_dir, pre_mode};
use crate::detect::is_git_commit;
use crate::gate::Gate;

/// The Conventional Commit gate.
pub(crate) struct CommitGate;
/// Its singleton in the ordered pre-tool table.
pub(crate) static COMMIT_GATE: CommitGate = CommitGate;

fn commit_prefix(message: &str) -> Option<&str> {
  let subject = message.lines().next().unwrap_or("");
  let len = subject.bytes().take_while(u8::is_ascii_lowercase).count();
  let (prefix, rest) = (subject.get(..len)?, subject.get(len..)?);
  if prefix.is_empty() || !(rest.starts_with(':') || rest.starts_with('(') && rest.contains("):")) {
    return None;
  }
  Some(prefix)
}

fn unknown_prefix(analysis: &ShellAnalysis, prefixes: &[String]) -> Option<String> {
  for command in &analysis.commands {
    let Some(git) = git_invocation(command).filter(|git| git.subcommand == Some("commit")) else {
      continue;
    };
    let subject = commit_messages(&git).into_iter().next().flatten();
    let bad = subject
      .as_deref()
      .and_then(commit_prefix)
      .filter(|prefix| !prefixes.iter().any(|allowed| allowed == prefix));
    if let Some(prefix) = bad {
      return Some(prefix.to_owned());
    }
  }
  None
}

fn word_char(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

fn names_word(source: &str, word: &str) -> bool {
  source.match_indices(word).any(|(at, _)| {
    let before = source.get(..at).and_then(|text| text.chars().next_back());
    let after = source
      .get(at + word.len()..)
      .and_then(|text| text.chars().next());
    !before.is_some_and(word_char) && !after.is_some_and(word_char)
  })
}

fn commits(analysis: &ShellAnalysis) -> bool {
  if !analysis.unknown {
    return is_git_commit(analysis);
  }
  names_word(&analysis.source, "git") && names_word(&analysis.source, "commit")
}

fn reminder(base: &str) -> String {
  format!(
    "BEFORE COMMITTING:\n1. Verify diff covers only expected scope (git diff --stat against {base})\n2. Save memory of significant decisions before committing.\nSkip only if already done this task."
  )
}

impl Gate for CommitGate {
  fn name(&self) -> &'static str {
    "commit-gate"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    let NormalizedEvent::ShellPre { tool, .. } = event else {
      return Ok(Decision::Allow);
    };
    if tool.name.as_str() != "Bash" {
      return Ok(Decision::Allow);
    }
    let analysis = command_analysis(ctx);
    if !commits(&analysis) {
      return Ok(Decision::Allow);
    }
    let mode = pre_mode(&gate_config(ctx), "commitGate", ctx, true);
    if mode == GateMode::Off {
      return Ok(Decision::Allow);
    }
    let cwd = ctx.cwd.unwrap_or(ctx.project_root);
    let base = base_branch(ctx.env, None, cwd);
    let prefixes = match gate_settings_dir(ctx) {
      Some(dir) => read_list(&dir.join(COMMIT_PREFIXES))?,
      None => Vec::new(),
    };
    let bad = (!prefixes.is_empty())
      .then(|| unknown_prefix(&analysis, &prefixes))
      .flatten();
    let Some(prefix) = bad else {
      return decided(GateMode::Advise, reminder(&base));
    };
    let reason = format!(
      "Unknown Conventional Commits prefix: \"{prefix}\". Allowed prefixes are in settings/commit-prefixes.txt. Base branch: {base}"
    );
    decided(mode, reason)
  }
}

#[cfg(test)]
#[path = "tests/commit_gate_test.rs"]
mod tests;
