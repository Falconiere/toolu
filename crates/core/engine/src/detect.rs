//! Git operations a command performs (`detect-git.ts`, the ports of
//! `is_git_push`, `is_git_commit`, `push_target_root` and `push_target_branch`),
//! answered from the command's `ShellAnalysis` rather than its text. Gates parse
//! once per event and pass the analysis in. Git facts come from `.git`; git is
//! spawned only where `toolu_runtime::git` defers to it.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_runtime::host::roots::Roots;
use toolu_shell::analysis::{ShellAnalysis, Tristate};
use toolu_shell::git::{GitInvocation, git_invocation, push_targets, runs_git_subcommand};
use toolu_state::git::current_branch;

use crate::gates::push_target::git_at;

/// Whether the command runs `git push`. A dynamic name (`$g push`) is unknown
/// to the shell layer and false here, as bash answers for the workflow gates; a
/// gate that must fail closed asks `runs_git_subcommand` itself.
pub fn is_git_push(analysis: &ShellAnalysis) -> bool {
  runs_git_subcommand(analysis, "push") == Tristate::Yes
}

/// Whether the command runs `git commit`; unknown is false, as for [`is_git_push`].
pub fn is_git_commit(analysis: &ShellAnalysis) -> bool {
  runs_git_subcommand(analysis, "commit") == Tristate::Yes
}

/// The toplevel of the repository the first push targets. Its whole `-C` chain
/// is replayed from `cwd`, each step relative to the last, as git applies them;
/// a dynamic step, or a directory with no repository, falls back to the cwd's
/// toplevel, then the host project root, then the cwd.
pub fn push_target_root(analysis: &ShellAnalysis, roots: &Roots, cwd: &Path) -> PathBuf {
  let env = roots.env();
  let pushes = push_targets(analysis);
  let chain = pushes
    .first()
    .map(|push| push.invocation.c_chain.as_slice())
    .unwrap_or_default();
  let replayed = (!chain.is_empty() && chain.iter().all(Option::is_some)).then(|| {
    chain
      .iter()
      .flatten()
      .fold(cwd.to_path_buf(), |dir, step| dir.join(step))
  });
  replayed
    .and_then(|dir| toplevel(env, &dir))
    .or_else(|| toplevel(env, cwd))
    .or_else(|| roots.project_root(Some(cwd)))
    .unwrap_or_else(|| cwd.to_path_buf())
}

/// The branch a push from `root` updates: the checked-out branch, or on a
/// detached or unborn HEAD the branch the first push's refspec names; `""` when
/// none can be named (a delete, bare `HEAD`, a wildcard, no or a dynamic refspec).
pub fn push_target_branch(analysis: &ShellAnalysis, root: &Path, env: &Env) -> String {
  let branch = current_branch(env, root);
  if !branch.is_empty() && branch != "HEAD" {
    return branch;
  }
  let pushes = push_targets(analysis);
  pushes
    .into_iter()
    .next()
    .and_then(|push| push.destination)
    .unwrap_or_default()
}

/// Builtins git runs in place of an alias of the same name.
// shortcut: the common builtins, not git's full list; a rarer one costs one `git config` spawn.
const BUILTINS: &[&str] = &[
  "add",
  "am",
  "apply",
  "bisect",
  "blame",
  "branch",
  "cat-file",
  "check-ignore",
  "check-ref-format",
  "checkout",
  "cherry-pick",
  "clean",
  "clone",
  "commit",
  "config",
  "describe",
  "diff",
  "diff-index",
  "diff-tree",
  "fetch",
  "for-each-ref",
  "format-patch",
  "fsck",
  "gc",
  "grep",
  "hash-object",
  "help",
  "init",
  "log",
  "ls-files",
  "ls-remote",
  "ls-tree",
  "merge",
  "merge-base",
  "mv",
  "name-rev",
  "notes",
  "pull",
  "push",
  "range-diff",
  "rebase",
  "reflog",
  "remote",
  "reset",
  "restore",
  "rev-list",
  "rev-parse",
  "revert",
  "rm",
  "shortlog",
  "show",
  "show-ref",
  "sparse-checkout",
  "stash",
  "status",
  "submodule",
  "switch",
  "symbolic-ref",
  "tag",
  "update-ref",
  "var",
  "version",
  "worktree",
];

/// Whether `text` names a push: the word `push`, or `send-pack`.
fn mentions_push(text: &str) -> bool {
  text.contains("send-pack")
    || text
      .split(|c: char| !c.is_ascii_alphanumeric())
      .any(|word| word == "push")
}

/// Whether a word may name an alias key: it names `alias.`, or is dynamic on a
/// line that mentions `push`.
fn alias_word(word: Option<&str>, mentioned: bool) -> bool {
  word.map_or(mentioned, |word| {
    word.to_ascii_lowercase().contains("alias.")
  })
}

/// Whether the invocation may define an alias: a `-c` or `--config-env`
/// global, or `git config`, setting an `alias.` key.
fn defines_alias(git: &GitInvocation<'_>, mentioned: bool) -> bool {
  let argv = &git.command.argv;
  let globals = argv
    .get(1..argv.len().saturating_sub(git.args.len() + 1))
    .unwrap_or_default();
  let mut words = globals.iter();
  while let Some(word) = words.next() {
    let value = match word.as_deref() {
      Some("-c" | "--config-env") => words.next().and_then(Option::as_deref),
      Some(word) if word.starts_with("--config-env=") => word.strip_prefix("--config-env="),
      Some(word) if word.starts_with("-c") => word.strip_prefix("-c"),
      _ => continue,
    };
    if alias_word(value, mentioned) {
      return true;
    }
  }
  git.subcommand == Some("config")
    && git
      .args
      .iter()
      .any(|word| alias_word(word.as_deref(), mentioned))
}

/// Whether `sub`, run in `dir`, is a configured alias that pushes.
fn configured_push(sub: &str, dir: &Path, env: &Env) -> bool {
  !BUILTINS.contains(&sub)
    && git_at(dir, &["config", "--get", &format!("alias.{sub}")], env)
      .is_some_and(|expansion| mentions_push(&expansion))
}

/// Where a line may push without a static `git push`: the directory (its static
/// `-C` chain from `cwd`) of the first command that may. A dynamic git
/// subcommand, `send-pack`, `http-push`, `subtree push` or a configured alias
/// that pushes, an `alias.` key set in the line or a `GIT_CONFIG_COUNT` or
/// `GIT_CONFIG_PARAMETERS` variable always may; a dynamic command name or config
/// key, or a line the parser could not read, when the line mentions `push`.
pub fn possible_push(analysis: &ShellAnalysis, cwd: &Path, env: &Env) -> Option<PathBuf> {
  let mentioned = mentions_push(&analysis.source);
  if mentioned && analysis.unknown {
    return Some(cwd.to_path_buf());
  }
  // GIT_CONFIG_COUNT (with its KEY/VALUE pairs) and GIT_CONFIG_PARAMETERS set any key;
  // assignments are not command words, so the line's text is searched.
  let env_config = ["GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS"]
    .iter()
    .any(|name| analysis.source.contains(name));
  analysis.commands.iter().find_map(|command| {
    let Some(git) = git_invocation(command) else {
      return (mentioned && matches!(command.argv.first(), Some(None))).then(|| cwd.to_path_buf());
    };
    let dir = git
      .c_chain
      .iter()
      .try_fold(cwd.to_path_buf(), |dir, step| {
        step.map(|step| dir.join(step))
      })
      .unwrap_or_else(|| cwd.to_path_buf());
    let pushes = match git.subcommand {
      None | Some("send-pack" | "http-push") => true,
      Some("subtree") => git.args.iter().any(|arg| arg.as_deref() == Some("push")),
      Some(sub) => env_config || defines_alias(&git, mentioned) || configured_push(sub, &dir, env),
    };
    pushes.then_some(dir)
  })
}

#[cfg(test)]
#[path = "tests/detect_test.rs"]
mod tests;
