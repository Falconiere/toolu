//! `cargo xtask deploy-guard`: the `PreToolUse` hook of the `/deploy` skill
//! (`.claude/skills/deploy/SKILL.md`). It blocks tag, tap and release publishing
//! unless `deploy-gate`'s receipt names the clean `HEAD`. Exit 2 is the block.

use std::path::Path;

use serde_json::Value;
use toolu_shell::analysis::{ShellAnalysis, ShellCommand};
use toolu_shell::git::git_invocation;

use crate::Verdict;
use crate::ci_diff::git;
use crate::options::Options;

/// Where `deploy-gate` records the commit it passed, under the root.
pub(crate) const RECEIPT: &str = "target/deploy/gate.json";

/// The `gh release` verbs that write to a release.
const RELEASE_WRITES: [&str; 4] = ["create", "upload", "edit", "delete"];

/// Allow or block the command in the hook payload on stdin.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let payload = toolu_protocol::stdin::read_stdin()
    .map_err(|err| format!("deploy-guard: cannot read stdin: {err}"))?;
  decide(&options.root, &payload).map(|()| Verdict::Clean)
}

/// `Ok` allows the payload's command; `Err` blocks it with the reason.
pub(crate) fn decide(root: &Path, payload: &str) -> Result<(), String> {
  let value: Value = serde_json::from_str(payload)
    .map_err(|err| format!("deploy-guard: the hook payload is not JSON: {err}"))?;
  let command = value
    .pointer("/tool_input/command")
    .and_then(Value::as_str)
    .ok_or("deploy-guard: the hook payload has no tool_input.command")?;
  if !publishes(&toolu_shell::analyze(command)) {
    return Ok(());
  }
  receipt_matches(root).map_err(|reason| {
    let first = command.lines().next().unwrap_or_default();
    format!(
      "deploy-guard: blocked `{first}`: {reason}; run cargo xtask deploy-gate on a clean HEAD first"
    )
  })
}

/// Whether the line may push a tag, push from another repository (`-C`), or
/// write a release or the release PR's label. A line that cannot be read, or a
/// dynamic name on a line that mentions publishing, counts: the guard fails closed.
fn publishes(analysis: &ShellAnalysis) -> bool {
  let mentions = analysis.source.contains("push") || analysis.source.contains("release");
  analysis.unknown
    || analysis
      .commands
      .iter()
      .any(|command| match command.argv.first() {
        Some(Some(name)) => match name.rsplit('/').next() {
          Some("git") => git_publishes(command, mentions),
          Some("gh") => gh_publishes(command),
          _ => false,
        },
        _ => mentions,
      })
}

fn git_publishes(command: &ShellCommand, mentions: bool) -> bool {
  let Some(git) = git_invocation(command) else {
    return false;
  };
  match git.subcommand {
    Some("push") => {
      // `texts` is aligned with `argv`, which ends with the push's arguments.
      let offset = command.argv.len().saturating_sub(git.args.len());
      let args = command.texts.get(offset..).unwrap_or_default();
      !git.c_chain.is_empty()
        || git.args.iter().any(Option::is_none)
        || args.iter().any(|arg| tag_like(arg))
    }
    Some(_) => false,
    None => mentions,
  }
}

/// A refspec or option that pushes tags: `refs/tags/…`, `--tags`, or `v<digit>…`.
fn tag_like(arg: &str) -> bool {
  let bare = arg.trim_start_matches('+');
  bare.contains("refs/tags/")
    || matches!(bare, "--tags" | "--follow-tags")
    || bare
      .strip_prefix('v')
      .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

fn gh_publishes(command: &ShellCommand) -> bool {
  let words = &command.texts;
  words.windows(2).enumerate().any(|(at, pair)| match pair {
    [word, next] if word == "release" => {
      RELEASE_WRITES.contains(&next.as_str()) || command.argv.get(at + 1) == Some(&None)
    }
    [word, next] if word == "pr" => {
      next == "edit" && words.iter().any(|word| word.contains("autorelease"))
    }
    _ => false,
  })
}

/// `Ok` when the receipt names `HEAD` and the worktree is clean.
fn receipt_matches(root: &Path) -> Result<(), String> {
  let text = std::fs::read_to_string(root.join(RECEIPT))
    .map_err(|err| format!("no gate receipt at {RECEIPT} ({err})"))?;
  let commit = serde_json::from_str::<Value>(&text)
    .ok()
    .and_then(|value| value.get("commit")?.as_str().map(str::to_owned))
    .filter(|commit| commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .ok_or_else(|| format!("the gate receipt {RECEIPT} is malformed"))?;
  let head = git(root, &["rev-parse", "HEAD"])?.trim().to_owned();
  if commit != head {
    return Err(format!(
      "the gate receipt is for {commit}, but HEAD is {head}"
    ));
  }
  if !git(root, &["status", "--porcelain"])?.trim().is_empty() {
    return Err("the worktree has uncommitted changes".to_owned());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/deploy_guard_test.rs"]
mod tests;
