//! The `semantic-git` pre-tool rule: a Bash/Shell command that commits or names
//! a new branch is checked against the semantic format. A failure is denied
//! (`gitBetter.mode: block`) or explained (`advise`); a message or name that
//! only exists at run time is never guessed, and earns an advisory instead.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};
use toolu_shell::git::{
  CommitArgs, GitInvocation, MessageOrigin, NewBranch, commit_args, git_invocation, new_branches,
};
use toolu_state::git::{base_branch, current_branch};

use crate::SPEC;
use crate::branch::{check_branch, example_branch};
use crate::format::{check_message, example_message, is_wip, with_trailers};
use crate::report::{CHECK_HINT, Finding, Target, render};
use crate::settings::{self, Mode, Settings};

/// The rule's manifest name.
pub(crate) const NAME: &str = "semantic-git";
/// The subcommands that can commit or name a branch.
const SUBCOMMANDS: [&str; 5] = ["commit", "checkout", "switch", "branch", "worktree"];

/// The compiled pre-tool rule.
struct SemanticGit;

/// `semantic-git` manifest's compiled rule.
pub const SEMANTIC_GIT: &dyn Rule = &SemanticGit;

/// What the rule made of one commit or branch.
enum Outcome {
  Failed(Finding),
  Unread(String),
}

fn command_of(event: &NormalizedEvent) -> Option<String> {
  if let NormalizedEvent::ShellPre { command, .. } = event {
    return Some(command.as_str().to_owned());
  }
  let NormalizedEvent::ToolPre { tool, .. } = event else {
    return None;
  };
  tool
    .input
    .get("command")
    .and_then(Value::as_str)
    .map(str::to_owned)
}

/// The directory a git invocation works in: `cwd`, then each static `-C`.
fn repo_dir(cwd: &Path, git: &GitInvocation<'_>) -> PathBuf {
  git
    .c_chain
    .iter()
    .fold(cwd.to_path_buf(), |dir, step| match step {
      Some(step) => dir.join(step),
      None => dir,
    })
}

/// The commit's message as git will write it, `Ok(None)` when it is not new
/// text, or why it cannot be read before the command runs.
fn message(args: &CommitArgs, dir: &Path) -> Result<Option<String>, String> {
  if args.origin != MessageOrigin::Written {
    return Ok(None);
  }
  let mut parts = Vec::new();
  for value in &args.messages {
    parts.push(value.clone().ok_or("its `-m` value expands at run time")?);
  }
  for file in &args.files {
    let text = match file.as_deref() {
      None => return Err("its `-F` path expands at run time".to_owned()),
      Some("-") => args.stdin.clone().ok_or("it is piped into `-F -`")?,
      Some(path) => fs::read_to_string(dir.join(path))
        .map_err(|error| format!("its file `{path}` cannot be read yet ({error})"))?,
    };
    parts.push(text);
  }
  if parts.is_empty() {
    return Err("it is written in an editor".to_owned());
  }
  let mut extra = Vec::new();
  for value in &args.trailers {
    extra.push(
      value
        .clone()
        .ok_or("a `--trailer` value expands at run time")?,
    );
  }
  Ok(Some(with_trailers(&parts.join("\n\n"), &extra)))
}

fn on_base(env: &Env, dir: &Path) -> bool {
  current_branch(env, dir) == base_branch(env, None, dir)
}

fn commit(args: &CommitArgs, settings: &Settings, env: &Env, dir: &Path) -> Option<Outcome> {
  let text = match message(args, dir) {
    Ok(Some(text)) => text,
    Ok(None) => return None,
    Err(why) => return Some(Outcome::Unread(format!("the commit message, as {why}"))),
  };
  let base = is_wip(&text) && on_base(env, dir);
  let violations = check_message(&text, settings, base);
  (!violations.is_empty()).then(|| {
    Outcome::Failed(Finding {
      target: Target::Message,
      name: String::new(),
      violations,
      example: example_message(&text),
    })
  })
}

fn exists(env: &Env, dir: &Path, name: &str) -> bool {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(dir.display().to_string());
  spec
    .argv
    .extend(["rev-parse", "--verify", "--quiet"].map(str::to_owned));
  spec.argv.push(format!("refs/heads/{name}"));
  spec.env = Some(env.clone());
  run(&spec).is_ok_and(|out| out.exit_code == 0)
}

/// The base branch, release-please's branches and branches that already exist
/// are not new names.
fn exempt(name: &str, env: &Env, dir: &Path) -> bool {
  name.starts_with("release-please--")
    || name == base_branch(env, None, dir)
    || exists(env, dir, name)
}

fn branch(found: &NewBranch, settings: &Settings, env: &Env, dir: &Path) -> Option<Outcome> {
  let Some(name) = &found.name else {
    return Some(Outcome::Unread(
      "the new branch name, as it expands at run time".to_owned(),
    ));
  };
  let violations = check_branch(name, settings);
  if violations.is_empty() || exempt(name, env, dir) {
    return None;
  }
  Some(Outcome::Failed(Finding {
    target: Target::Branch,
    name: name.clone(),
    violations,
    example: example_branch(name, settings),
  }))
}

fn decide(outcomes: Vec<Outcome>, mode: Mode) -> Decision {
  let mut failed = Vec::new();
  let mut unread = Vec::new();
  for outcome in outcomes {
    match outcome {
      Outcome::Failed(finding) => failed.push(finding),
      Outcome::Unread(why) => unread.push(why),
    }
  }
  let text = if !failed.is_empty() {
    let headline = match mode {
      Mode::Block => "git-better blocked this command: it is not semantic.",
      Mode::Advise | Mode::Off => {
        "git-better: this is not semantic (gitBetter.mode is advise, so it was not stopped)."
      }
    };
    render(headline, &failed)
  } else if !unread.is_empty() {
    format!(
      "git-better cannot check {} before the command runs. {CHECK_HINT}",
      unread.join("; or ")
    )
  } else {
    return Decision::Allow;
  };
  let Ok(text) = Text::new(text) else {
    return Decision::Allow;
  };
  match mode {
    Mode::Block if !failed.is_empty() => Decision::Deny { reason: text },
    Mode::Off => Decision::Allow,
    Mode::Block | Mode::Advise => Decision::Advisory { message: text },
  }
}

impl Rule for SemanticGit {
  fn spec(&self) -> &'static str {
    SPEC
  }
  fn name(&self) -> &'static str {
    NAME
  }
  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPre
  }

  fn applies(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> bool {
    event
      .tool()
      .is_some_and(|tool| matches!(tool.name.as_str(), "Bash" | "Shell"))
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    let Some(command) = command_of(event) else {
      return Decision::Allow;
    };
    let analysis = toolu_shell::analyze(&command);
    let gits: Vec<GitInvocation<'_>> = analysis
      .commands
      .iter()
      .filter_map(git_invocation)
      .filter(|git| git.subcommand.is_some_and(|sub| SUBCOMMANDS.contains(&sub)))
      .collect();
    if gits.is_empty() {
      return Decision::Allow;
    }
    let cwd = ctx.cwd.unwrap_or(ctx.project_root);
    let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
    let settings = settings::load(&roots, Some(cwd), ctx.plugin_root);
    if settings.mode == Mode::Off {
      return Decision::Allow;
    }
    let mut outcomes = Vec::new();
    for git in &gits {
      let dir = repo_dir(cwd, git);
      if let Some(args) = commit_args(git) {
        outcomes.extend(commit(&args, &settings, ctx.env, &dir));
      }
      for found in new_branches(git) {
        outcomes.extend(branch(&found, &settings, ctx.env, &dir));
      }
    }
    decide(outcomes, settings.mode)
  }
}

#[cfg(test)]
#[path = "tests/pre_test.rs"]
mod tests;
