//! What the Rust crate answers for one fixture case, in the shape the case's
//! `expected` (or `bash`) uses: the port of `actual` in `issue-283.test.ts` and
//! of the `bats-parity.test.ts` branches.

use std::time::Instant;

use serde_json::{Value, json};
use toolu_shell::analysis::Tristate;
use toolu_shell::git::{commit_messages, git_invocation, runs_git_subcommand};
use toolu_shell::writes::write_targets;

use crate::cases::{Res, field};
use crate::decide::decide;
use crate::git_repo::{push_branch_answer, push_root_answer};
use crate::lists::strings;

/// The function a case exercises: `fn` in the bats file, `kind` in the #283 file.
pub(crate) fn kind(case: &Value) -> Res<&str> {
  field(case, "fn").or_else(|_| field(case, "kind"))
}

/// The TypeScript answer: `expected` when the case has one, else `bash`.
fn want(case: &Value) -> Res<Value> {
  let raw = case.get("expected").or_else(|| case.get("bash"));
  let raw = raw.ok_or_else(|| format!("no expected or bash in {case}"))?;
  Ok(sorted_if_paths(kind(case)?, raw.clone()))
}

/// Write targets are compared as sets, as both TypeScript tests do.
fn sorted_if_paths(kind: &str, value: Value) -> Value {
  let Value::Array(mut items) = value else {
    return value;
  };
  if kind == "bash_write_targets" {
    items.sort_by_key(Value::to_string);
  }
  Value::Array(items)
}

fn is_git(command: &str, sub: &str) -> bool {
  runs_git_subcommand(&toolu_shell::analyze(command), sub) == Tristate::Yes
}

fn written_paths(command: &str) -> Value {
  let analysis = toolu_shell::analyze(command);
  let paths: Vec<String> = write_targets(&analysis)
    .into_iter()
    .map(|target| target.path.unwrap_or(target.text))
    .collect();
  json!(paths)
}

fn first_commit_messages(command: &str) -> Value {
  let analysis = toolu_shell::analyze(command);
  let commit = analysis
    .commands
    .iter()
    .filter_map(git_invocation)
    .find(|git| git.subcommand == Some("commit"));
  json!(commit.map(|git| commit_messages(&git)).unwrap_or_default())
}

fn commands_of(command: &str) -> Value {
  let analysis = toolu_shell::analyze(command);
  let commands: Vec<Value> = analysis
    .commands
    .iter()
    .map(|cmd| json!({ "argv": cmd.argv, "pipeline": cmd.pipeline.index }))
    .collect();
  json!(commands)
}

fn exit_proves_of(command: &str) -> Value {
  let analysis = toolu_shell::analyze(command);
  let pairs: Vec<Value> = analysis
    .commands
    .iter()
    .map(|cmd| json!([cmd.argv.first().cloned().flatten(), cmd.exit_proves]))
    .collect();
  json!(pairs)
}

/// Within budget reads back as the budget; over it, the diff shows the real time.
fn latency_of(command: &str, budget: &Value) -> Res<Value> {
  let max = budget.get("maxMs");
  let max_ms = max.and_then(Value::as_f64);
  let (max, max_ms) = max
    .zip(max_ms)
    .ok_or_else(|| format!("no maxMs in {budget}"))?;
  let started = Instant::now();
  let push = is_git(command, "push");
  let elapsed = started.elapsed().as_secs_f64() * 1000.0;
  let reported = if elapsed <= max_ms {
    max.clone()
  } else {
    json!(elapsed)
  };
  Ok(json!({ "maxMs": reported, "push": push }))
}

fn decided(case: &Value, command: &str) -> Res<Value> {
  let analysis = toolu_shell::analyze(command);
  let verdict = decide(&analysis, &strings(case, "allow")?, &strings(case, "deny")?);
  Ok(json!(verdict))
}

fn answer(case: &Value) -> Res<Value> {
  let command = field(case, "command")?;
  match kind(case)? {
    "is_git_push" => Ok(json!(is_git(command, "push"))),
    "is_git_commit" => Ok(json!(is_git(command, "commit"))),
    "bash_write_targets" => Ok(sorted_if_paths(
      "bash_write_targets",
      written_paths(command),
    )),
    "bash_commands_decide" => decided(case, command),
    "push_target_root" => push_root_answer(case),
    "push_target_branch" => push_branch_answer(case),
    "commit_messages" => Ok(first_commit_messages(command)),
    "commands" => Ok(commands_of(command)),
    "exit_proves" => Ok(exit_proves_of(command)),
    "latency" => latency_of(command, case.get("expected").unwrap_or(&Value::Null)),
    other => Err(format!("unhandled kind {other} in {case}")),
  }
}

/// Why the crate and TypeScript disagree on `case`, or `None` when they do not.
pub(crate) fn mismatch(case: &Value) -> Res<Option<String>> {
  let (got, wanted) = (answer(case)?, want(case)?);
  let (kind, command) = (kind(case)?, field(case, "command")?);
  if got != wanted {
    return Ok(Some(format!(
      "{kind} {command:?}\n  want {wanted}\n  got  {got}"
    )));
  }
  // The baseline of a live #283 case is the defect: bash must differ from the fix.
  let live = field(case, "oracle").is_ok_and(|oracle| oracle == "live");
  let defect = live && case.get("bash") == case.get("expected");
  Ok(defect.then(|| format!("{kind} {command:?}\n  its bash baseline equals the fix")))
}
