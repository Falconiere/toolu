//! The `semantic-git` pre-tool rule: a Bash/Shell command that commits or names
//! a new branch is checked against the semantic format. A failure is denied
//! (`gitBetter.mode: block`) or explained (`advise`); a message or name that
//! only exists at run time is never guessed, and earns an advisory instead.
//! Config values the plugin could not read are reported with the decision.

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
use toolu_state::git::base_branch;

use crate::SPEC;
use crate::branch::{check_branch, example_branch};
use crate::context::on_base_branch;
use crate::format::{check_message, example_message, is_wip};
use crate::message::{Line, message};
use crate::report::{CHECK_HINT, Finding, Target, render};
use crate::settings::{self, Mode, Settings};

/// The rule's manifest name.
pub(crate) const NAME: &str = "semantic-git";

/// The compiled pre-tool rule.
struct SemanticGit;

/// `semantic-git` manifest's compiled rule.
pub const SEMANTIC_GIT: &dyn Rule = &SemanticGit;

/// What the rule made of one commit or branch.
enum Outcome {
  Failed(Finding),
  Unread(String),
}

/// A commit or new branch to check, in the directory git runs in (`None` when
/// a `-C` expands at run time).
enum Item {
  Commit(Option<PathBuf>, CommitArgs),
  Branch(Option<PathBuf>, NewBranch),
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

/// The directory a git invocation works in: `cwd`, then each `-C`; `None` when
/// one expands at run time.
fn repo_dir(cwd: &Path, git: &GitInvocation<'_>) -> Option<PathBuf> {
  let mut dir = cwd.to_path_buf();
  for step in &git.c_chain {
    dir = dir.join(step.as_deref()?);
  }
  Some(dir)
}

/// Every commit with new text and every new branch name in the line.
fn items(analysis: &toolu_shell::analysis::ShellAnalysis, cwd: &Path) -> Vec<Item> {
  let mut found = Vec::new();
  for git in analysis.commands.iter().filter_map(git_invocation) {
    let dir = repo_dir(cwd, &git);
    if let Some(args) = commit_args(&git).filter(|args| args.origin == MessageOrigin::Written) {
      found.push(Item::Commit(dir.clone(), args));
    }
    found.extend(
      new_branches(&git)
        .into_iter()
        .map(|name| Item::Branch(dir.clone(), name)),
    );
  }
  found
}

fn commit(
  args: &CommitArgs,
  settings: &Settings,
  env: &Env,
  dir: &Path,
  line: &Line,
) -> Option<Outcome> {
  let text = match message(args, dir, line) {
    Ok(Some(text)) => text,
    Ok(None) => return None,
    Err(why) => return Some(Outcome::Unread(format!("the commit message, as {why}"))),
  };
  let base = is_wip(&text) && on_base_branch(env, dir);
  let violations = check_message(&text, settings, base);
  (!violations.is_empty()).then(|| {
    Outcome::Failed(Finding {
      target: Target::Message,
      name: String::new(),
      violations,
      example: example_message(&text, settings),
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

fn outcome(item: &Item, settings: &Settings, env: &Env, line: &Line) -> Option<Outcome> {
  let dynamic = |what: &str| {
    Some(Outcome::Unread(format!(
      "{what}, as its `-C` directory expands at run time"
    )))
  };
  match item {
    Item::Commit(Some(dir), args) => commit(args, settings, env, dir, line),
    Item::Commit(None, _) => dynamic("the commit message"),
    Item::Branch(Some(dir), found) => branch(found, settings, env, dir),
    Item::Branch(None, _) => dynamic("the new branch name"),
  }
}

fn decide(outcomes: Vec<Outcome>, settings: &Settings) -> Decision {
  let mut failed = Vec::new();
  let mut unread = Vec::new();
  for outcome in outcomes {
    match outcome {
      Outcome::Failed(finding) => failed.push(finding),
      Outcome::Unread(why) => unread.push(why),
    }
  }
  let mut text = if !failed.is_empty() {
    let headline = match settings.mode {
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
    String::new()
  };
  if !settings.notes.is_empty() {
    let separator = if text.is_empty() { "" } else { "\n" };
    text = format!(
      "{text}{separator}git-better config: {}",
      settings.notes.join("; ")
    );
  }
  let Ok(text) = Text::new(text) else {
    return Decision::Allow;
  };
  if settings.mode == Mode::Block && !failed.is_empty() {
    Decision::Deny { reason: text }
  } else {
    Decision::Advisory { message: text }
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
    let shell = event
      .tool()
      .is_some_and(|tool| matches!(tool.name.as_str(), "Bash" | "Shell"));
    shell && command_of(event).is_some_and(|command| command.contains("git"))
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    let Some(command) = command_of(event) else {
      return Decision::Allow;
    };
    let analysis = toolu_shell::analyze(&command);
    let cwd = ctx.cwd.unwrap_or(ctx.project_root);
    let work = items(&analysis, cwd);
    if work.is_empty() {
      return Decision::Allow;
    }
    let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
    let settings = settings::load(&roots, Some(cwd), ctx.plugin_root);
    if settings.mode == Mode::Off {
      return Decision::Allow;
    }
    let line = Line::of(&analysis);
    let outcomes = work
      .iter()
      .filter_map(|item| outcome(item, &settings, ctx.env, &line))
      .collect();
    decide(outcomes, &settings)
  }
}

#[cfg(test)]
#[path = "tests/pre_test.rs"]
mod tests;
