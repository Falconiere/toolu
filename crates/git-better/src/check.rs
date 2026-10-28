//! `toolu git-better check`: the rule's checks on a message and a branch name,
//! as text or one `gitBetterCheck` document, exit 1 when one fails.

use std::path::Path;

use clap::ArgMatches;
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::git::{base_branch, current_branch};

use crate::branch::{check_branch, example_branch};
use crate::cli::env_of;
use crate::format::{check_message, example_message, is_wip};
use crate::report::{Finding, Target, render};
use crate::settings::{self, Settings};

fn read_message(args: &ArgMatches, ctx: &Ctx) -> Result<Option<String>, String> {
  if let Some(parts) = args.get_many::<String>("message") {
    let parts: Vec<&str> = parts.map(String::as_str).collect();
    return Ok(Some(parts.join("\n\n")));
  }
  match args.get_one::<String>("message-file").map(String::as_str) {
    None => Ok(None),
    Some("-") => ctx
      .stdin
      .clone()
      .map(Some)
      .ok_or_else(|| "cannot read the message from stdin".to_owned()),
    Some(path) => std::fs::read_to_string(path)
      .map(Some)
      .map_err(|error| format!("cannot read {path}: {error}")),
  }
}

/// The findings for what the verb was given.
pub(crate) fn findings(
  message: Option<&str>,
  branch: Option<&str>,
  settings: &Settings,
  on_base: bool,
) -> Vec<Finding> {
  let mut found = Vec::new();
  if let Some(text) = message {
    let violations = check_message(text, settings, on_base);
    if !violations.is_empty() {
      found.push(Finding {
        target: Target::Message,
        name: String::new(),
        violations,
        example: example_message(text),
      });
    }
  }
  if let Some(name) = branch {
    let violations = check_branch(name, settings);
    if !violations.is_empty() {
      found.push(Finding {
        target: Target::Branch,
        name: name.to_owned(),
        violations,
        example: example_branch(name, settings),
      });
    }
  }
  found
}

fn document(found: &[Finding], notes: &[String]) -> String {
  let findings: Vec<_> = found
    .iter()
    .map(|finding| {
      let violations: Vec<_> = finding
        .violations
        .iter()
        .map(|violation| json!({ "code": violation.code, "message": violation.message }))
        .collect();
      json!({
        "target": finding.target.as_str(),
        "name": finding.name,
        "violations": violations,
        "example": finding.example,
      })
    })
    .collect();
  json!({
    "namespace": crate::PLUGIN,
    "verb": "check",
    "ok": found.is_empty(),
    "findings": findings,
    "notes": notes,
  })
  .to_string()
}

/// Run `check`.
pub(crate) fn check(args: &ArgMatches, ctx: &Ctx) -> Outcome {
  let message = match read_message(args, ctx) {
    Ok(message) => message,
    Err(why) => return Outcome::failed(Exit::Failure, why),
  };
  let branch = args.get_one::<String>("branch").map(String::as_str);
  if message.is_none() && branch.is_none() {
    let why = "give a message (--message or --message-file) or --branch";
    return Outcome::failed(Exit::Usage, why.to_owned());
  }
  let env = env_of(ctx);
  let cwd = std::env::current_dir().unwrap_or_default();
  let roots = Roots::new(env.clone(), ctx.host);
  let settings = settings::load(&roots, Some(&cwd), None);
  let wip = message.as_deref().is_some_and(is_wip);
  let base = wip && on_base(&env, &cwd);
  let found = findings(message.as_deref(), branch, &settings, base);
  let exit = if found.is_empty() {
    Exit::Success
  } else {
    Exit::Failure
  };
  let stdout = if ctx.json {
    document(&found, &settings.notes)
  } else if found.is_empty() {
    "semantic: ok".to_owned()
  } else {
    render("git-better: not semantic.", &found)
  };
  let stderr = (!ctx.json && !settings.notes.is_empty()).then(|| settings.notes.join("\n"));
  Outcome {
    exit,
    stdout: Some(stdout),
    stderr,
  }
}

fn on_base(env: &Env, cwd: &Path) -> bool {
  current_branch(env, cwd) == base_branch(env, None, cwd)
}

#[cfg(test)]
#[path = "tests/check_test.rs"]
mod tests;
