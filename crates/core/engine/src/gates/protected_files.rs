//! The protected-files PreToolUse gate, over edit paths and parsed shell writes.

use std::collections::HashSet;

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::gate_mode::{GateMode, guardrail_warning};
use toolu_runtime::config::settings::{PROTECTED_FILES, read_list};
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::writes::write_targets;

use super::gate_paths::{expand, repo_relative};
use super::pattern::Pattern;
use super::{command_analysis, decided, file_path, gate_config, gate_settings_dir, pre_mode};
use crate::gate::Gate;

/// The protected-files built-in gate.
pub(crate) struct ProtectedFiles;
/// Its singleton in the ordered pre-tool table.
pub(crate) static PROTECTED: ProtectedFiles = ProtectedFiles;

struct Rule {
  pattern: String,
  tests: Vec<Pattern>,
  by_name: Option<Pattern>,
}

impl Rule {
  fn new(pattern: String) -> Rule {
    let mut tests = vec![Pattern::new(&pattern)];
    if !pattern.starts_with("**/") {
      tests.push(Pattern::new(&format!("**/{pattern}")));
    }
    let by_name = (!pattern.contains('/')).then(|| Pattern::new(&pattern));
    Rule {
      pattern,
      tests,
      by_name,
    }
  }

  fn fits(&self, rel: &str) -> bool {
    let basename = rel.rsplit('/').next().unwrap_or(rel);
    self.tests.iter().any(|test| test.matches(rel))
      || self
        .by_name
        .as_ref()
        .is_some_and(|test| test.matches(basename))
  }
}

struct Hit {
  candidate: String,
  rel: String,
  matched: String,
  shell: bool,
}

fn candidates(event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Vec<String> {
  if matches!(event, NormalizedEvent::ShellPre { .. }) {
    let cwd = ctx.cwd.unwrap_or(ctx.project_root);
    let mut seen = HashSet::new();
    return write_targets(&command_analysis(ctx))
      .into_iter()
      .flat_map(|target| match (target.path, target.pattern) {
        (Some(path), _) => vec![path],
        (None, Some(pattern)) => expand(&pattern, cwd),
        (None, None) if !target.text.is_empty() => vec![target.text],
        (None, None) => Vec::new(),
      })
      .filter(|path| !path.is_empty() && seen.insert(path.clone()))
      .collect();
  }
  let Some(tool) = event.tool() else {
    return Vec::new();
  };
  if !matches!(tool.name.as_str(), "Edit" | "Write" | "MultiEdit") {
    return Vec::new();
  }
  let path = file_path(event);
  if path.is_empty() {
    Vec::new()
  } else {
    vec![path.to_owned()]
  }
}

fn first_hit(
  paths: Vec<String>,
  rules: &[Rule],
  ctx: &RuleContext<'_>,
  shell: bool,
) -> Option<Hit> {
  for candidate in paths {
    let rel = repo_relative(&candidate, ctx.project_root);
    let matched = rules.iter().find(|rule| rule.fits(&rel));
    if let Some(rule) = matched {
      return Some(Hit {
        candidate,
        rel,
        matched: rule.pattern.clone(),
        shell,
      });
    }
  }
  None
}

fn detail_for(rel: &str) -> &'static str {
  const EXAMPLE: &str = "This is an example/template env file. It is committed on purpose, so it should carry placeholders and never live values — it is guarded because a real credential pasted here is a credential published to the repo.";
  const SECRET: &str = "This is a secrets file. Approving lets an agent read or rewrite live credentials, and anything it writes here can leak into logs, commits, or a diff you push.";
  const GIT: &str = "This is git's internal state. Approving lets an agent rewrite refs, hooks, or config — including hooks that run on your machine at every commit.";
  const HOOK: &str = "This is toolu's own enforcement code — the hooks that run every other gate. Approving lets an agent edit the thing that is supposed to be watching it, which is how a guardrail gets quietly switched off.";
  const DEFAULT: &str = "This path is listed in settings/protected-files.txt because edits to it are hard to notice and expensive to get wrong.";
  [
    ("@(*.env.example|*.env.template|*.env.sample)", EXAMPLE),
    ("@(.env|.env.*|*secrets*)", SECRET),
    ("@(.git/*|*/.git/*)", GIT),
    ("@(*hooks/*|*skills/*)", HOOK),
  ]
  .into_iter()
  .find_map(|(pattern, detail)| super::pattern::matches(pattern, rel).then_some(detail))
  .unwrap_or(DEFAULT)
}

fn reason(mode: GateMode, hit: &Hit) -> String {
  let detail = detail_for(&hit.rel);
  let headline = if hit.shell {
    format!(
      "This command would WRITE to {}, a protected path (matches \"{}\").",
      hit.candidate, hit.matched
    )
  } else {
    format!(
      "Claude is trying to edit {}, a protected path (matches \"{}\").",
      hit.candidate, hit.matched
    )
  };
  match mode {
    GateMode::Ask => guardrail_warning(&headline, detail),
    GateMode::Advise => format!(
      "Protected path {} (matches \"{}\"). {detail} The write was NOT stopped — gates.protectedFiles.mode is 'advise'.",
      hit.candidate, hit.matched
    ),
    GateMode::Block | GateMode::Off => format!(
      "{headline} {detail} Blocked by gates.protectedFiles.mode='block' (see plugins/toolu/hooks/docs/gates.md)."
    ),
  }
}

impl Gate for ProtectedFiles {
  fn name(&self) -> &'static str {
    "protected-files"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    let Some(dir) = gate_settings_dir(ctx) else {
      return Ok(Decision::Allow);
    };
    let rules: Vec<_> = read_list(&dir.join(PROTECTED_FILES))?
      .into_iter()
      .map(Rule::new)
      .collect();
    if rules.is_empty() {
      return Ok(Decision::Allow);
    }
    let shell = matches!(event, NormalizedEvent::ShellPre { .. });
    let Some(hit) = first_hit(candidates(event, ctx), &rules, ctx, shell) else {
      return Ok(Decision::Allow);
    };
    let mode = pre_mode(&gate_config(ctx), "protectedFiles", ctx, shell);
    decided(mode, reason(mode, &hit))
  }
}

#[cfg(test)]
#[path = "tests/protected_files_test.rs"]
mod tests;
