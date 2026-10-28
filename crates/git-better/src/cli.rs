//! The `toolu git-better` namespace: `check` runs the rule's checks on a message
//! or branch name before the agent commits or creates it.

use std::path::Path;

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::git::{base_branch, current_branch};

use crate::branch::{check_branch, example_branch};
use crate::format::{check_message, example_message, is_wip};
use crate::report::{Finding, Target, render};
use crate::settings::{self, Settings};

/// The namespace and its verbs.
pub(crate) fn command() -> Command {
  Command::new("git-better")
    .about("Semantic branches and commits that agents can read")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      Command::new("check")
        .about(
          "Check a commit message or branch name against the semantic format; exit 1 when it fails",
        )
        .arg(
          Arg::new("message")
            .short('m')
            .long("message")
            .value_name("TEXT")
            .action(ArgAction::Append)
            .help("A message paragraph, as `git commit -m` takes it; repeat for more"),
        )
        .arg(
          Arg::new("message-file")
            .short('F')
            .long("message-file")
            .value_name("FILE")
            .conflicts_with("message")
            .help("Read the message from FILE, or from stdin with `-`"),
        )
        .arg(
          Arg::new("branch")
            .long("branch")
            .value_name("NAME")
            .help("A new branch name to check"),
        ),
    )
}

/// Whether the verb reads its message from stdin.
pub(crate) fn needs_stdin(matches: &ArgMatches) -> bool {
  matches.subcommand().is_some_and(|(verb, args)| {
    verb == "check" && args.get_one::<String>("message-file").map(String::as_str) == Some("-")
  })
}

fn env_of(ctx: &Ctx) -> Env {
  let env = Env::process();
  match &ctx.config_dir {
    Some(dir) => env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy()),
    None => env,
  }
}

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

fn check(args: &ArgMatches, ctx: &Ctx) -> Outcome {
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

/// Run a `toolu git-better` verb.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand() {
    Some(("check", args)) => check(args, ctx),
    _ => Outcome::failed(Exit::Usage, "a verb is required".to_owned()),
  }
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
