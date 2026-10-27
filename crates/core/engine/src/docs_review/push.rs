//! Restrict reviewed pushes to one explicitly named HEAD source.
use super::{base, git};
use crate::gates::push_target::git_at;
use std::path::Path;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_shell::analysis::{CommandOrigin, ShellAnalysis};
use toolu_shell::git::{GitPush, push_targets};

const SHAPE: &str = "push one reviewed HEAD using git push <remote> HEAD:<branch>";

/// The flags a reviewed push may carry.
const FLAGS: [&str; 12] = [
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
  "--no-follow-tags",
];

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

/// The line's only command, when it is a plain static `git push`.
fn single_push(analysis: &ShellAnalysis) -> Option<GitPush<'_>> {
  if analysis.unknown || analysis.commands.len() != 1 {
    return None;
  }
  let push = push_targets(analysis).into_iter().next()?;
  let cmd = push.invocation.command;
  let plain = cmd.origin == CommandOrigin::Line
    && cmd.wrappers.is_empty()
    && cmd.words == cmd.argv
    && cmd
      .text
      .trim_start()
      .strip_prefix("git")
      .is_some_and(|rest| rest.starts_with(char::is_whitespace))
    && cmd.redirects.is_empty()
    && globals(&cmd.argv);
  plain.then_some(push)
}

/// The push's operands, when every flag is one of [`FLAGS`].
fn operands<'a>(push: &GitPush<'a>) -> Option<Vec<&'a str>> {
  let mut operands = Vec::new();
  for arg in push.invocation.args {
    let arg = arg.as_deref()?;
    if !arg.starts_with('-') {
      operands.push(arg);
    } else if !FLAGS.contains(&arg) {
      return None;
    }
  }
  Some(operands)
}

fn source<'a>(operands: &[&'a str]) -> Option<(&'a str, &'a str)> {
  let [remote, spec] = operands else {
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

fn commit(root: &Path, env: &Env, revision: &str) -> Option<String> {
  let revision = format!("{revision}^{{commit}}");
  git_at(
    root,
    &["rev-parse", "--verify", "--end-of-options", &revision],
    env,
  )
  .map(|sha| sha.trim().to_owned())
  .filter(|sha| !sha.is_empty())
}

/// Verify the only pushed source resolves to the reviewed HEAD.
pub(super) fn reviewed_head(
  analysis: &ShellAnalysis,
  root: &Path,
  env: &Env,
) -> Result<(), String> {
  let push = single_push(analysis).ok_or(SHAPE)?;
  let operands = operands(&push).ok_or(SHAPE)?;
  let (src, dst) = source(&operands).ok_or(SHAPE)?;
  let no_follow = push
    .invocation
    .args
    .iter()
    .any(|arg| arg.as_deref() == Some("--no-follow-tags"));
  if !no_follow && !tags_disabled(root, env) {
    return Err(
      "push.followTags is true, so the push would also send tags; add --no-follow-tags".into(),
    );
  }
  if !branch_refs(root, env, src, dst) {
    return Err(SHAPE.into());
  }
  match (commit(root, env, "HEAD"), commit(root, env, src)) {
    (Some(head), Some(source)) if head == source => Ok(()),
    _ => Err(SHAPE.into()),
  }
}

/// Whether the line is one plain push whose every source commit (`HEAD` for an
/// implicit refspec) is already in the base branch: nothing to review.
pub(super) fn nothing_to_review(analysis: &ShellAnalysis, roots: &Roots, root: &Path) -> bool {
  let Some(push) = single_push(analysis) else {
    return false;
  };
  let Some(operands) = operands(&push) else {
    return false;
  };
  let Ok(base) = base(roots, root) else {
    return false;
  };
  // shortcut: an implicit push is taken to send HEAD, as push.default=simple does; matching pushes more.
  let specs = operands
    .get(1..)
    .filter(|specs| !specs.is_empty())
    .unwrap_or(&["HEAD"][..]);
  specs.iter().all(|spec| {
    let spec = spec.strip_prefix('+').unwrap_or(spec);
    let src = spec.split_once(':').map_or(spec, |(src, _)| src);
    !src.is_empty()
      && !src.contains('*')
      && commit(root, roots.env(), src)
        .is_some_and(|sha| git(roots, root, &["merge-base", "--is-ancestor", &sha, &base]).is_ok())
  })
}

#[cfg(test)]
#[path = "tests/push_test.rs"]
mod tests;
