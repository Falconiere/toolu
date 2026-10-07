//! Codex agent profiles. Stdout matches `setup.ts` aside from the program name.

use std::path::{Path, PathBuf};

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

use super::{env_of, opencode};

mod apply;
mod classify;

const MARKER: &str = "# Managed by toolu. Install and update with $toolu:setup.";
const USAGE: &str =
  "Usage: toolu setup agents preview | install [--force] | remove --yes [--force]";
const OPENCODE: &str = "\
toolu setup agents: OpenCode registers toolu's agents itself (toolu-quick-task, \
toolu-deep-explore, toolu-research-agent, toolu-implementer, toolu-architect); there are no \
Codex profiles to install. Set agent.<id>.model in opencode.json to pin a model, or disable: \
true to drop one.";
const REFUSE_REMOVE: &str = "REFUSED removal requires --yes after explicit user confirmation";
const REFUSE_CONFLICT: &str =
  "REFUSED unmanaged profile conflict; preview it and confirm install/remove --force";

struct Profile {
  name: &'static str,
  model: &'static str,
  effort: &'static str,
  sandbox: &'static str,
  body: &'static str,
}

const PROFILES: [Profile; 5] = [
  Profile {
    name: "quick-task",
    model: "gpt-5.6-luna",
    effort: "medium",
    sandbox: "read-only",
    body: include_str!("../../../../plugins/toolu/assets/agents/quick-task.toml"),
  },
  Profile {
    name: "deep-explore",
    model: "gpt-5.6-terra",
    effort: "medium",
    sandbox: "read-only",
    body: include_str!("../../../../plugins/toolu/assets/agents/deep-explore.toml"),
  },
  Profile {
    name: "research-agent",
    model: "gpt-5.6-terra",
    effort: "medium",
    sandbox: "read-only",
    body: include_str!("../../../../plugins/toolu/assets/agents/research-agent.toml"),
  },
  Profile {
    name: "implementer",
    model: "gpt-5.6-terra",
    effort: "medium",
    sandbox: "workspace-write",
    body: include_str!("../../../../plugins/toolu/assets/agents/implementer.toml"),
  },
  Profile {
    name: "architect",
    model: "gpt-5.6-sol",
    effort: "high",
    sandbox: "read-only",
    body: include_str!("../../../../plugins/toolu/assets/agents/architect.toml"),
  },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
  Install,
  Unchanged,
  Update,
  Conflict,
  Absent,
  Remove,
}

struct Plan {
  command: &'static str,
  force: bool,
  confirmed: bool,
}

/// The script's usage line, exit 2.
pub(super) fn usage() -> Outcome {
  Outcome::failed(Exit::Blocked, USAGE.to_owned())
}

/// Preview, install or remove. `OpenCode` refuses before any other parse.
pub(super) fn run(ctx: &Ctx, args: &[String]) -> Outcome {
  let env = env_of(ctx);
  if opencode(ctx, &env) {
    return Outcome::failed(Exit::Blocked, OPENCODE.to_owned());
  }
  let Some(plan) = parse(args) else {
    return usage();
  };
  match execute(&env, &plan) {
    Ok(done) => done,
    Err(message) => Outcome {
      exit: Exit::Failure,
      stdout: None,
      stderr: Some(format!("toolu setup: {message}")),
    },
  }
}

fn parse(args: &[String]) -> Option<Plan> {
  let (command, flags) = args.split_first()?;
  let command = match command.as_str() {
    "preview" => "preview",
    "install" => "install",
    "remove" => "remove",
    _ => return None,
  };
  let mut force = false;
  let mut confirmed = false;
  for flag in flags {
    match flag.as_str() {
      "--force" => force = true,
      "--yes" => confirmed = true,
      _ => return None,
    }
  }
  Some(Plan {
    command,
    force,
    confirmed,
  })
}

fn execute(env: &Env, plan: &Plan) -> Result<Outcome, String> {
  let templates = template_dir(env);
  let agents = codex_agents(env)?;
  check_templates(templates.as_deref())?;
  let mut stdout = format!("TARGET {agents}");
  let actions = classify::planned(&mut stdout, templates.as_deref(), &agents, plan.command);
  if let Some(done) = stop(&stdout, plan, &actions) {
    return Ok(done);
  }
  let backup = match apply::prepare_backup(env, &agents, &actions) {
    Ok(backup) => backup,
    Err(message) => return Ok(failed_stdout(stdout, &message)),
  };
  let job = apply::Job::new(
    templates.as_deref(),
    &agents,
    backup.as_deref(),
    plan.command,
    &actions,
  );
  if let Err(message) = apply::commit(&mut stdout, &job) {
    return Ok(failed_stdout(stdout, &message));
  }
  Ok(Outcome::data(stdout))
}

fn stop(stdout: &str, plan: &Plan, actions: &[Action]) -> Option<Outcome> {
  if plan.command == "preview" {
    return Some(Outcome::data(preview_text(stdout, actions)));
  }
  if plan.command == "remove" && !plan.confirmed {
    return Some(blocked(stdout, REFUSE_REMOVE));
  }
  if actions.contains(&Action::Conflict) && !plan.force {
    return Some(blocked(stdout, REFUSE_CONFLICT));
  }
  None
}

fn preview_text(stdout: &str, actions: &[Action]) -> String {
  if actions.contains(&Action::Conflict) {
    format!("{stdout}\nNOTICE use install --force only after confirming each conflict")
  } else {
    stdout.to_owned()
  }
}

fn blocked(stdout: &str, stderr: &str) -> Outcome {
  Outcome {
    exit: Exit::Blocked,
    stdout: Some(stdout.to_owned()),
    stderr: Some(stderr.to_owned()),
  }
}

fn failed_stdout(stdout: String, message: &str) -> Outcome {
  Outcome {
    exit: Exit::Failure,
    stdout: Some(stdout),
    stderr: Some(format!("toolu setup: {message}")),
  }
}

fn template_dir(env: &Env) -> Option<PathBuf> {
  env.get("TOOLU_AGENT_TEMPLATE_DIR").map(PathBuf::from)
}

fn codex_agents(env: &Env) -> Result<String, String> {
  let root = env
    .get("CODEX_HOME")
    .map(ToOwned::to_owned)
    .or_else(|| env.get("HOME").map(|home| format!("{home}/.codex")));
  root
    .map(|root| format!("{root}/agents"))
    .ok_or_else(|| "CODEX_HOME and HOME are both unset".to_owned())
}

fn check_templates(dir: Option<&Path>) -> Result<(), String> {
  for profile in &PROFILES {
    let text = template_text(dir, profile)?;
    if !classify::valid(&text, profile) {
      return Err(format!(
        "invalid agent template: {}",
        template_label(dir, profile.name)
      ));
    }
  }
  Ok(())
}

fn template_label(dir: Option<&Path>, name: &str) -> String {
  match dir {
    Some(dir) => dir.join(format!("{name}.toml")).display().to_string(),
    None => name.to_owned(),
  }
}

fn template_text(dir: Option<&Path>, profile: &Profile) -> Result<String, String> {
  let Some(dir) = dir else {
    return Ok(profile.body.to_owned());
  };
  let path = dir.join(format!("{}.toml", profile.name));
  std::fs::read_to_string(&path)
    .map_err(|_err| format!("invalid agent template: {}", path.display()))
}

#[cfg(test)]
#[path = "tests/agents_test.rs"]
mod tests;
