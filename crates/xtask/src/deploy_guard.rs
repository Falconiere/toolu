//! `cargo xtask deploy-guard`: the `PreToolUse` hook of the `/deploy` skill
//! (`.claude/skills/deploy/SKILL.md`). It blocks tag, tap and release publishing
//! unless `deploy-gate`'s receipt names the clean `HEAD` at `origin/main`, any
//! write to that receipt, and tagging another commit. Exit 2 is the block.

use std::path::Path;

use serde_json::Value;
use toolu_shell::analysis::{ShellAnalysis, ShellCommand, Word};
use toolu_shell::git::git_invocation;

use crate::Verdict;
use crate::ci_diff::git;
use crate::options::Options;

/// Where `deploy-gate` records the commit it passed, under the root.
pub(crate) const RECEIPT: &str = "target/deploy/gate.json";

/// The `gh release` verbs that write to a release.
const RELEASE_WRITES: [&str; 5] = ["create", "upload", "edit", "delete", "delete-asset"];

/// The `git tag` options that take the next word as their value.
const TAG_VALUES: [&str; 8] = [
  "-m",
  "-F",
  "-u",
  "--message",
  "--file",
  "--local-user",
  "--cleanup",
  "--trailer",
];

/// Allow or block the tool call in the hook payload on stdin.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let payload = toolu_protocol::stdin::read_stdin()
    .map_err(|err| format!("deploy-guard: cannot read stdin: {err}"))?;
  decide(&options.root, &payload).map(|()| Verdict::Clean)
}

/// `Ok` allows the payload's tool call; `Err` blocks it with the reason.
pub(crate) fn decide(root: &Path, payload: &str) -> Result<(), String> {
  let value: Value = serde_json::from_str(payload)
    .map_err(|err| format!("deploy-guard: the hook payload is not JSON: {err}"))?;
  if value
    .get("tool_name")
    .and_then(Value::as_str)
    .unwrap_or("Bash")
    != "Bash"
  {
    return file_tool(&value);
  }
  let command = value
    .pointer("/tool_input/command")
    .and_then(Value::as_str)
    .ok_or("deploy-guard: the hook payload has no tool_input.command")?;
  let blocked = |what: &str, reason: &str| format!("deploy-guard: blocked `{what}`: {reason}");
  if command.contains("gate.json") {
    return Err(blocked(
      command,
      "only cargo xtask deploy-gate writes the gate receipt",
    ));
  }
  let analysis = toolu_shell::analyze(command);
  if let Some(tag) = retagging(&analysis) {
    return Err(blocked(tag, "a release tag must point at HEAD"));
  }
  let Some(culprit) = publishing(root, &analysis) else {
    return Ok(());
  };
  receipt_matches(root).map_err(|reason| {
    blocked(
      culprit,
      &format!("{reason}; run cargo xtask deploy-gate on a clean HEAD at origin/main first"),
    )
  })
}

/// A file tool (`Write`, `Edit`, `MultiEdit`, `NotebookEdit`) may touch any path but the receipt.
fn file_tool(value: &Value) -> Result<(), String> {
  let path = ["/tool_input/file_path", "/tool_input/notebook_path"]
    .iter()
    .find_map(|field| value.pointer(field).and_then(Value::as_str))
    .ok_or("deploy-guard: the hook payload has no file path")?;
  if Path::new(path).ends_with(RECEIPT) {
    return Err(format!(
      "deploy-guard: blocked a write to {path}: only cargo xtask deploy-gate writes the gate receipt"
    ));
  }
  Ok(())
}

/// The command that may push a tag, push another repository, or write a release
/// or the release PR's label. A line that cannot be read, or a dynamic name on a
/// line that mentions publishing, counts: the guard fails closed.
fn publishing<'a>(root: &Path, analysis: &'a ShellAnalysis) -> Option<&'a str> {
  let source = analysis.source.as_str();
  let mentions = source.contains("push") || source.contains("release");
  if analysis.unknown {
    return mentions.then_some(source);
  }
  // `cd`, `GIT_DIR` or `GIT_WORK_TREE` can point a push at the tap.
  let elsewhere = source.contains("GIT_DIR")
    || source.contains("GIT_WORK_TREE")
    || analysis
      .commands
      .iter()
      .any(|command| matches!(name(command), Some("cd" | "pushd")));
  analysis
    .commands
    .iter()
    .find(|command| match command.argv.first() {
      None => false,
      Some(None) => mentions,
      Some(Some(_)) => match name(command) {
        Some("git") => git_publishes(root, command, mentions, elsewhere),
        Some("gh") => gh_publishes(&command.argv, mentions),
        _ => false,
      },
    })
    .map(|command| command.text.as_str())
}

/// The base name of the command's program.
fn name(command: &ShellCommand) -> Option<&str> {
  let program = command.argv.first()?.as_deref()?;
  Path::new(program).file_name()?.to_str()
}

fn git_publishes(root: &Path, command: &ShellCommand, mentions: bool, elsewhere: bool) -> bool {
  let Some(invocation) = git_invocation(command) else {
    return false;
  };
  match invocation.subcommand {
    Some("push") => {
      elsewhere
        || !invocation.c_chain.is_empty()
        || invocation.args.iter().any(Option::is_none)
        || command.argv.iter().flatten().any(|word| {
          word.starts_with("--git-dir")
            || word.starts_with("--work-tree")
            || word.to_ascii_lowercase().contains("push.followtags")
            || tag_like(word)
        })
        || git(root, &["config", "--type=bool", "--get", "push.followTags"])
          .is_ok_and(|value| value.trim() == "true")
    }
    Some(_) => false,
    None => mentions,
  }
}

/// A push word that sends tags: `--tags`, `--follow-tags`, `--mirror`, the `tag`
/// keyword, or a refspec side under `refs/tags/`, `tags/`, or named `v<digit>…`.
fn tag_like(word: &str) -> bool {
  let spec = word.trim_start_matches('+');
  matches!(spec, "--tags" | "--follow-tags" | "--mirror" | "tag")
    || spec.split(':').any(|side| {
      side.contains("refs/tags/")
        || side.starts_with("tags/")
        || side
          .strip_prefix('v')
          .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
    })
}

/// `gh release <write>`, and `gh pr|issue edit` touching the `autorelease`
/// label; a dynamic group, verb or `edit` argument fails closed.
fn gh_publishes(argv: &[Word], mentions: bool) -> bool {
  match (argv.get(1), argv.get(2)) {
    (Some(None), _) => mentions,
    (Some(Some(group)), verb) if group == "release" => match verb {
      Some(Some(verb)) => RELEASE_WRITES.contains(&verb.as_str()),
      Some(None) => true,
      None => false,
    },
    (Some(Some(group)), Some(Some(verb)))
      if matches!(group.as_str(), "pr" | "issue") && verb == "edit" =>
    {
      argv.iter().any(|word| {
        word
          .as_deref()
          .is_none_or(|word| word.contains("autorelease"))
      })
    }
    _ => false,
  }
}

/// The `git tag` that creates a tag on a commit other than `HEAD`.
fn retagging(analysis: &ShellAnalysis) -> Option<&str> {
  analysis
    .commands
    .iter()
    .find(|command| {
      git_invocation(command)
        .is_some_and(|git| git.subcommand == Some("tag") && tags_another_commit(git.args))
    })
    .map(|command| command.text.as_str())
}

/// Whether `git tag <args>` creates a tag whose target is given and is not `HEAD`.
fn tags_another_commit(args: &[Word]) -> bool {
  let mut operands = Vec::new();
  let mut words = args.iter();
  while let Some(word) = words.next() {
    match word.as_deref() {
      Some("-l" | "--list" | "-d" | "--delete" | "-v" | "--verify") => return false,
      Some(option) if TAG_VALUES.contains(&option) => {
        words.next();
      }
      Some(flag) if flag.starts_with('-') => {}
      operand => operands.push(operand),
    }
  }
  operands
    .get(1)
    .is_some_and(|target| *target != Some("HEAD"))
}

/// `Ok` when the receipt names `HEAD`, `HEAD` is `origin/main`, and the worktree is clean.
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
  let main = git(
    root,
    &["rev-parse", "-q", "--verify", "refs/remotes/origin/main"],
  )?;
  if main.trim() != head {
    return Err(format!("HEAD {head} is not origin/main {}", main.trim()));
  }
  if !git(root, &["status", "--porcelain"])?.trim().is_empty() {
    return Err("the worktree has uncommitted changes".to_owned());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/deploy_guard_test.rs"]
mod tests;
