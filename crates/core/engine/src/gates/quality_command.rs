//! Quality commands in a parsed command line (`quality-command.ts`).
//! gate-status records the quality gate only for a command that actually runs
//! one of these, never for text that names one (#283 item 6). The forms are
//! `gate-status.sh`'s regex, read from each simple command's argv (wrappers
//! already removed) instead of from the line's text; behind a package runner or
//! as a script given to `bash`/`sh` they still count.

use toolu_shell::analysis::{CommandOrigin, ShellAnalysis, ShellCommand, Word};

/// One simple command that runs a quality command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QualityCommand<'a> {
  /// The command as parsed.
  pub(crate) command: &'a ShellCommand,
  /// The quality command as `gate-status.sh` names it, e.g. `bun run lint`.
  pub(crate) label: String,
}

const BUN_SCRIPTS: [&str; 15] = [
  "check",
  "check:fix",
  "check:duplication",
  "ts:check",
  "ts:check:fix",
  "rust:check",
  "rust:test",
  "check-types",
  "lint",
  "lint:fix",
  "format",
  "format:check",
  "format:fix",
  "build",
  "test",
];
const CARGO: [&str; 4] = ["clippy", "test", "build", "nextest"];
const JS_TOOLS: [&str; 3] = ["vitest", "jest", "tsc"];
const TS_CHECK: [&str; 2] = ["./scripts/ts-check.sh", "scripts/ts-check.sh"];
const WRAPPER_FILES: [&str; 3] = ["check.sh", "test.sh", "format.sh"];
const PACKAGE_RUNNERS: [&str; 4] = ["npx", "bunx", "pnpx", "yarn"];
const SCRIPT_SHELLS: [&str; 2] = ["bash", "sh"];

/// `path.basename`: the last component, trailing slashes ignored.
fn basename(name: &str) -> &str {
  let trimmed = name.trim_end_matches('/');
  trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// The word at `at` when it is static.
fn word(argv: &[Word], at: usize) -> Option<&str> {
  argv.get(at).and_then(Option::as_deref)
}

/// `(?:^|/)(tools/[A-Za-z0-9_.-]+/(?:check|test|format)\.sh)$`: the captured
/// `tools/<dir>/<file>` of `name`.
fn wrapper_script(name: &str) -> Option<&str> {
  let (head, file) = name.rsplit_once('/')?;
  let (prefix, dir) = head.rsplit_once('/')?;
  let dir_ok = !dir.is_empty()
    && dir
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
  let at = prefix.len().checked_sub("tools".len())?;
  let anchored = prefix.ends_with("tools") && (at == 0 || prefix.get(..at)?.ends_with('/'));
  (WRAPPER_FILES.contains(&file) && dir_ok && anchored).then(|| name.get(at..))?
}

fn bun_label(argv: &[Word]) -> Option<String> {
  match (word(argv, 1), word(argv, 2)) {
    (Some("test"), _) => Some("bun test".to_owned()),
    (Some("run"), Some(script)) if BUN_SCRIPTS.contains(&script) => {
      Some(format!("bun run {script}"))
    }
    _ => None,
  }
}

fn cargo_label(argv: &[Word]) -> Option<String> {
  let toolchain = word(argv, 1).is_some_and(|first| first.starts_with('+'));
  let sub = word(argv, if toolchain { 2 } else { 1 })?;
  CARGO.contains(&sub).then(|| format!("cargo {sub}"))
}

/// The label of `argv` run directly, when it is a quality command.
fn direct_label(argv: &[Word]) -> Option<String> {
  let name = word(argv, 0)?;
  if let Some(script) = wrapper_script(name) {
    return Some(script.to_owned());
  }
  if TS_CHECK.contains(&name) {
    return Some(name.to_owned());
  }
  let base = basename(name);
  if JS_TOOLS.contains(&base) {
    return Some(base.to_owned());
  }
  match base {
    "bun" => bun_label(argv),
    "cargo" => cargo_label(argv),
    _ => None,
  }
}

/// `argv` from the first word at or after `from` that is not an option; empty
/// when there is none. A dynamic word is no option.
fn after_options(argv: &[Word], from: usize) -> &[Word] {
  let at = argv
    .iter()
    .enumerate()
    .position(|(i, word)| i >= from && !word.as_deref().is_some_and(|w| w.starts_with('-')));
  at.and_then(|at| argv.get(at..)).unwrap_or_default()
}

/// The command a package runner or script shell runs for `argv`.
fn runner_target(argv: &[Word]) -> Option<&[Word]> {
  let base = basename(word(argv, 0)?);
  let first = word(argv, 1);
  if PACKAGE_RUNNERS.contains(&base) {
    return Some(after_options(argv, 1));
  }
  if (base == "bun" && first == Some("x")) || (base == "pnpm" && first == Some("exec")) {
    return Some(after_options(argv, 2));
  }
  let script = first?;
  (SCRIPT_SHELLS.contains(&base) && !script.starts_with('-')).then(|| argv.get(1..))?
}

fn label_of(command: &ShellCommand) -> Option<String> {
  if command.origin == CommandOrigin::Function {
    return None;
  }
  direct_label(&command.argv).or_else(|| runner_target(&command.argv).and_then(direct_label))
}

/// Every simple command in the line that runs a quality command, in execution order.
pub(crate) fn quality_commands(analysis: &ShellAnalysis) -> Vec<QualityCommand<'_>> {
  analysis
    .commands
    .iter()
    .filter_map(|command| label_of(command).map(|label| QualityCommand { command, label }))
    .collect()
}

#[cfg(test)]
#[path = "tests/quality_command_test.rs"]
mod tests;
