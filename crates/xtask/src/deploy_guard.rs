//! `cargo xtask deploy-guard`: the `PreToolUse` hook of the `/deploy` skill
//! (`.claude/skills/deploy/SKILL.md`). It blocks tag, tap and release publishing
//! unless `deploy-gate`'s receipt names the clean `HEAD` at `origin/main`, any
//! write to that receipt or to the guard's snapshot, and tagging another commit.
//! Exit 2 is the block. Reading the line is `deploy_line`'s.

use std::path::{Component, Path};

use serde_json::Value;

use crate::Verdict;
use crate::ci_diff::git;
use crate::deploy_line::{Place, protected_write, publishing};
use crate::deploy_tag::{foreign_tag, retagging};
use crate::options::Options;

/// Where `deploy-gate` records the commit it passed, under the root.
pub(crate) const RECEIPT: &str = "target/deploy/gate.json";

/// Where `deploy-gate` snapshots this binary for the hook, under the root, so a
/// later checkout or build failure cannot stop the guard from running.
pub(crate) const SNAPSHOT: &str = ".claude/tmp/deploy-guard";

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
  let analysis = toolu_shell::analyze(command);
  if let Some(write) = protected_write(&analysis, protected, &["gate.json", "deploy-guard"]) {
    return Err(blocked(
      write,
      "only cargo xtask deploy-gate writes the receipt and the guard",
    ));
  }
  if let Some(tag) = retagging(&analysis) {
    return Err(blocked(tag, "a release tag must point at HEAD"));
  }
  let place = Place {
    root,
    cwd: value.get("cwd").and_then(Value::as_str),
  };
  let Some(culprit) = publishing(&place, &analysis) else {
    return Ok(());
  };
  if let Some(tag) = foreign_tag(root, &analysis) {
    return Err(blocked(tag, "a release tag must point at HEAD"));
  }
  receipt_matches(root).map_err(|reason| {
    blocked(
      culprit,
      &format!("{reason}; run cargo xtask deploy-gate on a clean HEAD at origin/main first"),
    )
  })
}

/// A file tool (`Write`, `Edit`, `MultiEdit`, `NotebookEdit`) may touch any path
/// but the receipt and the guard snapshot.
fn file_tool(value: &Value) -> Result<(), String> {
  let path = ["/tool_input/file_path", "/tool_input/notebook_path"]
    .iter()
    .find_map(|field| value.pointer(field).and_then(Value::as_str))
    .ok_or("deploy-guard: the hook payload has no file path")?;
  if protected(path) {
    return Err(format!(
      "deploy-guard: blocked a write to {path}: only cargo xtask deploy-gate writes the receipt and the guard"
    ));
  }
  Ok(())
}

/// Whether `path`, with `.` and `..` resolved and case folded (APFS ignores
/// case), ends in the receipt or the guard snapshot.
fn protected(path: &str) -> bool {
  let mut parts: Vec<String> = Vec::new();
  for component in Path::new(path).components() {
    match component {
      Component::ParentDir => {
        parts.pop();
      }
      Component::Normal(part) => parts.push(part.to_string_lossy().to_lowercase()),
      Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
    }
  }
  [RECEIPT, SNAPSHOT].iter().any(|guarded| {
    let tail: Vec<String> = guarded.split('/').map(str::to_owned).collect();
    parts.ends_with(&tail)
  })
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
