//! Restrict reviewed pushes to one explicitly named HEAD source.
use crate::gates::push_target::git_at;
use std::path::Path;
use toolu_runtime::env::Env;
use toolu_shell::analysis::{CommandOrigin, ShellAnalysis, Word};
use toolu_shell::git::{GitInvocation, git_invocation, push_targets};

/// Whether `text` holds `push` as a whole word.
fn mentions_push(text: &str) -> bool {
  text
    .split(|c: char| !c.is_ascii_alphanumeric())
    .any(|word| word == "push")
}

/// Whether the invocation may define an alias: a dynamic or `alias.` global
/// (`-c`, `--config-env`), or `git config` with a dynamic or `alias.` argument.
fn defines_alias(git: &GitInvocation<'_>) -> bool {
  let aliased = |words: &[Word]| {
    words.iter().any(|word| {
      word
        .as_deref()
        .is_none_or(|word| word.to_ascii_lowercase().contains("alias."))
    })
  };
  let argv = &git.command.argv;
  let globals = argv
    .get(1..argv.len().saturating_sub(git.args.len() + 1))
    .unwrap_or_default();
  aliased(globals) || (git.subcommand == Some("config") && aliased(git.args))
}

/// Whether the subcommand is an alias configured, where it runs, to push.
fn configured_push(git: &GitInvocation<'_>, cwd: &Path, env: &Env) -> bool {
  let Some(sub) = git.subcommand.filter(|sub| *sub != "push") else {
    return false;
  };
  let dir = git
    .c_chain
    .iter()
    .try_fold(cwd.to_path_buf(), |dir, step| {
      step.map(|step| dir.join(step))
    })
    .unwrap_or_else(|| cwd.to_path_buf());
  git_at(&dir, &["config", "--get", &format!("alias.{sub}")], env)
    .is_some_and(|expansion| mentions_push(&expansion))
}

/// Whether the line may push without a static `push` subcommand: a dynamic
/// command, subcommand or script, or an alias defined in the line, when the line
/// mentions `push`; or a configured alias that expands to one.
pub(super) fn possible_push(analysis: &ShellAnalysis, cwd: &Path, env: &Env) -> bool {
  let mentioned = mentions_push(&analysis.source);
  analysis
    .commands
    .iter()
    .any(|command| match git_invocation(command) {
      Some(git) => {
        (mentioned && (git.subcommand.is_none() || defines_alias(&git)))
          || configured_push(&git, cwd, env)
      }
      None => mentioned && matches!(command.argv.first(), Some(None)),
    })
}

fn globals(argv: &[Option<String>]) -> bool {
  let mut words = argv.iter().skip(1);
  loop {
    match words.next().and_then(Option::as_deref) {
      Some("push") => return true,
      Some("-C") if words.next().and_then(Option::as_deref).is_some() => {}
      _ => return false,
    }
  }
}

fn source(args: &[Option<String>]) -> Option<(&str, &str)> {
  let mut operands = Vec::new();
  for arg in args {
    let arg = arg.as_deref()?;
    if arg.starts_with('-') {
      if ![
        "-u",
        "--set-upstream",
        "-f",
        "--force",
        "--force-with-lease",
        "--no-verify",
        "--atomic",
        "--quiet",
        "-q",
        "--verbose",
        "-v",
      ]
      .contains(&arg)
      {
        return None;
      }
    } else {
      operands.push(arg);
    }
  }
  let [remote, spec] = operands.as_slice() else {
    return None;
  };
  if remote.is_empty() || spec.is_empty() {
    return None;
  }
  let spec = spec.strip_prefix('+').unwrap_or(spec);
  let (src, dst) = spec.split_once(':').unwrap_or((spec, spec));
  if src.is_empty() || dst.is_empty() || src.contains('*') || dst.contains('*') || dst.contains(':')
  {
    return None;
  }
  Some((src, dst))
}

fn branch_refs(root: &Path, env: &Env, src: &str, dst: &str) -> bool {
  let branch = src.strip_prefix("refs/heads/").unwrap_or(src);
  if src != "HEAD"
    && git_at(
      root,
      &[
        "show-ref",
        "--verify",
        "--quiet",
        &format!("refs/heads/{branch}"),
      ],
      env,
    )
    .is_none()
  {
    return false;
  }
  let destination = dst.strip_prefix("refs/heads/").unwrap_or(dst);
  if dst.starts_with("refs/") && !dst.starts_with("refs/heads/") {
    return false;
  }
  if dst != "HEAD" && git_at(root, &["check-ref-format", "--branch", destination], env).is_none() {
    return false;
  }
  true
}

fn tags_disabled(root: &Path, env: &Env) -> bool {
  git_at(
    root,
    &[
      "config",
      "--bool",
      "--default",
      "false",
      "--get",
      "push.followTags",
    ],
    env,
  )
  .is_some_and(|value| value.trim() == "false")
}

/// Verify the only pushed source resolves to the reviewed HEAD.
pub(super) fn reviewed_head(analysis: &ShellAnalysis, root: &Path, env: &Env) -> bool {
  if analysis.unknown || analysis.commands.len() != 1 || !tags_disabled(root, env) {
    return false;
  }
  let pushes = push_targets(analysis);
  let [push] = pushes.as_slice() else {
    return false;
  };
  let cmd = push.invocation.command;
  if cmd.origin != CommandOrigin::Line
    || !cmd.wrappers.is_empty()
    || cmd.words != cmd.argv
    || !cmd
      .text
      .trim_start()
      .strip_prefix("git")
      .is_some_and(|rest| rest.starts_with(char::is_whitespace))
    || !cmd.redirects.is_empty()
    || !globals(&cmd.argv)
  {
    return false;
  }
  let Some((src, dst)) = source(push.invocation.args) else {
    return false;
  };
  if !branch_refs(root, env, src, dst) {
    return false;
  }
  let revision = format!("{src}^{{commit}}");
  let current = git_at(root, &["rev-parse", "--verify", "HEAD^{commit}"], env);
  let source_commit = git_at(
    root,
    &["rev-parse", "--verify", "--end-of-options", &revision],
    env,
  );
  matches!((current, source_commit), (Some(a), Some(b)) if !a.trim().is_empty() && a == b)
}

#[cfg(test)]
#[path = "tests/push_test.rs"]
mod tests;
