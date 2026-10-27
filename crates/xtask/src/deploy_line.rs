//! How `deploy-guard` reads one Bash line, through toolu-shell's parse: the
//! command that publishes a release and a write to a path the guard protects.
//! A line it cannot read fails closed. Tags are `deploy_tag`'s.

use std::path::Path;

use toolu_shell::analysis::{ShellAnalysis, ShellCommand, Word};
use toolu_shell::git::git_invocation;
use toolu_shell::writes::write_targets;

use crate::ci_diff::git;

/// The `gh release` verbs that write to a release (`new` is `create`'s alias).
const RELEASE_WRITES: [&str; 6] = ["create", "new", "upload", "edit", "delete", "delete-asset"];

/// The `gh` flags that take the next word as their value.
const GH_VALUES: [&str; 3] = ["-R", "--repo", "--hostname"];

/// Where the line runs: the toolu repository and the payload's `cwd`.
pub(crate) struct Place<'a> {
  pub(crate) root: &'a Path,
  pub(crate) cwd: Option<&'a str>,
}

/// The command that may push a tag, push another repository, or write a release
/// or the release PR's label. A line that cannot be read, or a dynamic name on a
/// line that mentions publishing, counts.
pub(crate) fn publishing<'a>(place: &Place<'_>, analysis: &'a ShellAnalysis) -> Option<&'a str> {
  let mentions = mentions(analysis, &["push", "release"]);
  if analysis.unknown {
    return mentions.then_some(analysis.source.as_str());
  }
  // `cd`, `env -C`, `GIT_DIR` or `GIT_WORK_TREE` can point a push at the tap.
  let elsewhere = analysis.source.contains("GIT_DIR")
    || analysis.source.contains("GIT_WORK_TREE")
    || analysis.commands.iter().any(|command| {
      matches!(name(command), Some("cd" | "pushd"))
        || (command.wrappers.iter().any(|wrapper| wrapper == "env")
          && command
            .words
            .iter()
            .flatten()
            .any(|word| word == "-C" || word.starts_with("--chdir")))
    });
  analysis
    .commands
    .iter()
    .find(|command| match command.argv.first() {
      None => false,
      Some(None) => mentions,
      Some(Some(_)) => match name(command) {
        Some("git") => git_publishes(place, command, mentions, elsewhere),
        Some("gh") => gh_publishes(&command.argv, mentions),
        _ => false,
      },
    })
    .map(|command| command.text.as_str())
}

/// Whether the source or any unquoted word contains one of `needles`.
fn mentions(analysis: &ShellAnalysis, needles: &[&str]) -> bool {
  let source = analysis.source.to_ascii_lowercase();
  needles.iter().any(|needle| {
    source.contains(needle)
      || analysis
        .commands
        .iter()
        .flat_map(|command| &command.texts)
        .any(|text| text.to_ascii_lowercase().contains(needle))
  })
}

/// The base name of the command's program.
pub(crate) fn name(command: &ShellCommand) -> Option<&str> {
  let program = command.argv.first()?.as_deref()?;
  Path::new(program).file_name()?.to_str()
}

fn git_publishes(
  place: &Place<'_>,
  command: &ShellCommand,
  mentions: bool,
  elsewhere: bool,
) -> bool {
  let Some(invocation) = git_invocation(command) else {
    return false;
  };
  match invocation.subcommand {
    Some("push") => {
      elsewhere
        || !invocation.c_chain.is_empty()
        || invocation.args.iter().any(Option::is_none)
        || command.argv.iter().flatten().any(|word| {
          word.starts_with("--git-dir")
            || word.starts_with("--work-tree")
            || word.to_ascii_lowercase().contains("push.followtags")
            || tag_like(word)
        })
        || away(place)
        || pushes_tags_by_config(place.root)
    }
    Some(sub) => aliases_push(place.root, command, sub),
    None => mentions,
  }
}

/// Whether `sub` is a git alias, set by `-c` on the line or in the root's config, that pushes.
fn aliases_push(root: &Path, command: &ShellCommand, sub: &str) -> bool {
  let key = format!("alias.{}", sub.to_ascii_lowercase());
  command.argv.iter().flatten().any(|word| {
    let word = word.to_ascii_lowercase();
    word.contains(&key) && word.contains("push")
  }) || git(root, &["config", "--get", &key]).is_ok_and(|value| value.contains("push"))
}

/// A push word that may send tags: `--tags`, `--follow-tags`, `--mirror`, the
/// `tag` keyword, or a refspec side under `refs/` but not `refs/heads/`, under
/// `tags/`, with a wildcard, or named `v<digit>…`.
pub(crate) fn tag_like(word: &str) -> bool {
  let spec = word.trim_start_matches('+');
  matches!(spec, "--tags" | "--follow-tags" | "--mirror" | "tag")
    || spec.split(':').any(|side| {
      side.contains('*')
        || (side.starts_with("refs/") && !side.starts_with("refs/heads/"))
        || side.starts_with("tags/")
        || side
          .strip_prefix('v')
          .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
    })
}

/// Whether the payload's `cwd` is in another repository than the root, as after
/// a `cd` in an earlier call. A `cwd` git cannot place counts as away.
fn away(place: &Place<'_>) -> bool {
  let Some(cwd) = place.cwd else {
    return false;
  };
  let toplevel = git(Path::new(cwd), &["rev-parse", "--show-toplevel"]);
  let same = toplevel
    .ok()
    .and_then(|top| std::fs::canonicalize(top.trim()).ok())
    == std::fs::canonicalize(place.root).ok();
  !same
}

/// Whether the root's git config makes a plain push send tags: `push.followTags`,
/// a mirror remote, or a remote `push` refspec that is tag-like.
fn pushes_tags_by_config(root: &Path) -> bool {
  let pattern = r"^(push\.followtags|remote\..*\.(mirror|push))$";
  let Ok(config) = git(root, &["config", "--get-regexp", pattern]) else {
    return false;
  };
  config.lines().any(|line| {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key.rsplit('.').next() {
      Some("followtags" | "mirror") => value.trim() == "true",
      Some("push") => tag_like(value.trim()),
      _ => false,
    }
  })
}

/// `gh release <write>`, and `gh pr|issue edit` touching the `autorelease`
/// label, past any `-R`/`--repo` flag; a dynamic group, verb or `edit` argument
/// fails closed.
fn gh_publishes(argv: &[Word], mentions: bool) -> bool {
  let mut operands = Vec::new();
  let mut words = argv.iter().skip(1);
  while let Some(word) = words.next() {
    match word.as_deref() {
      Some(flag) if GH_VALUES.contains(&flag) => {
        words.next();
      }
      Some(flag) if flag.starts_with('-') => {}
      operand => operands.push(operand),
    }
  }
  match (operands.first(), operands.get(1)) {
    (Some(None), _) => mentions,
    (Some(Some("release")), Some(Some(verb))) => RELEASE_WRITES.contains(verb),
    (Some(Some("release")), Some(None)) => true,
    (Some(Some("pr" | "issue")), Some(Some("edit"))) => argv.iter().any(|word| {
      word
        .as_deref()
        .is_none_or(|word| word.contains("autorelease"))
    }),
    _ => false,
  }
}

/// The text of a write to a path `protected` names; a dynamic write target on
/// a line that mentions one of `names` counts.
pub(crate) fn protected_write<'a>(
  analysis: &'a ShellAnalysis,
  protected: impl Fn(&str) -> bool,
  names: &[&str],
) -> Option<&'a str> {
  let mentioned = mentions(analysis, names);
  write_targets(analysis)
    .into_iter()
    .find(|target| target.path.as_deref().map_or(mentioned, &protected))
    .map(|target| {
      target
        .command
        .map_or(analysis.source.as_str(), |command| command.text.as_str())
    })
    .or_else(|| {
      analysis
        .commands
        .iter()
        .find(|command| name(command) == Some("ln") && links(command, &protected, mentioned))
        .map(|command| command.text.as_str())
    })
}

/// Whether an `ln` operand, or the link it would make inside another operand, is
/// protected: a link at the receipt forges it, and a link to it writes through.
fn links(command: &ShellCommand, protected: &impl Fn(&str) -> bool, mentioned: bool) -> bool {
  let words: Vec<Option<&str>> = command
    .argv
    .iter()
    .skip(1)
    .map(Option::as_deref)
    .filter(|word| !word.is_some_and(|word| word.starts_with('-')))
    .collect();
  words.iter().any(|word| {
    word.map_or(mentioned, |word| {
      let base = Path::new(word).file_name().and_then(|base| base.to_str());
      protected(word)
        || base.is_some_and(|base| {
          words
            .iter()
            .flatten()
            .any(|dir| protected(&format!("{dir}/{base}")))
        })
    })
  })
}

#[cfg(test)]
#[path = "tests/deploy_line_test.rs"]
mod tests;
