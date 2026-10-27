//! Repository conventions from the base ref, within `budget::RULES_MAX_BYTES`.
//! Every blob is read with `git cat-file` at the caller-supplied base commit
//! (the shape stage's merge-base), never from HEAD or the working tree, so a
//! branch cannot soften its own review. Rule text is reference data for the
//! prompt builder (#666); nothing here feeds a verdict.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;
use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_runtime::process::{Output, Spec, run};

use crate::budget::RULES_MAX_BYTES;
use crate::contract::Omitted;

/// Convention basenames probed at the root and at touched ancestors: the
/// `RULES_GLOB` equivalent (`**/<name>` restricted to touched directories).
const BASENAMES: [&str; 4] = [
  "AGENTS.md",
  "CLAUDE.md",
  "CONTRIBUTING.md",
  "CONVENTIONS.md",
];
/// The repo prompt override (#666), read from the base like any rule file.
const PROMPT_PATH: &str = ".github/code-review-prompt.md";
/// Why a whole file was left out for size.
const CAP_REASON: &str = "exceeds rules byte cap";
/// One byte past the cap, so an over-cap blob is known without reading it all.
const READ_LIMIT: usize = RULES_MAX_BYTES + 1;

/// One gathered convention file, verbatim from the base ref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleFile {
  /// Repository-relative path at the base ref.
  pub path: String,
  /// The file's exact base-ref text.
  pub text: String,
}

/// The conventions of one review: what fit, what was dropped, and whether the
/// base could be read at all. `bytes` counts included texts; the rendered
/// `dropped` and `notices` lines travel along so #666 charges them too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bundle {
  /// Whether the base resolved and was read; `false` means no rule text.
  pub available: bool,
  /// Included root then nested files, in inclusion order.
  pub files: Vec<RuleFile>,
  /// The base-ref prompt override, when present and within budget.
  pub prompt_override: Option<RuleFile>,
  /// Whole files left out (cap, encoding, or read failure), with reasons.
  pub dropped: Vec<Omitted>,
  /// Bundle-level notices; holds the `rules unavailable: …` line when the
  /// base could not be read.
  pub notices: Vec<String>,
  /// Included text bytes (files plus prompt override), capped.
  pub bytes: usize,
}

/// A failure that prevents gathering from running at all. An unreadable base
/// is not an error: it returns an unavailable `Bundle` instead.
#[derive(Debug, PartialEq, Eq)]
pub enum RulesError {
  /// Git could not run, or the root is not inside a Git work tree.
  Git(String),
}

/// Gather the base ref's convention files for the prompt builder. `base` is a
/// commit or ref to read at (callers pass the shape stage's merge-base) and
/// `touched` the diff's repo-relative file paths; nested rules are probed only
/// at their ancestors, nearest first.
///
/// # Errors
/// Returns `RulesError` when Git cannot run or `root` is outside a work tree.
/// A base that does not resolve yields an unavailable bundle, never HEAD text.
pub fn gather(
  env: &Env,
  root: &Path,
  base: &str,
  touched: &[String],
) -> Result<Bundle, RulesError> {
  if base.is_empty() || base.starts_with('-') {
    return Ok(unavailable(&format!("invalid base ref {base:?}")));
  }
  let top =
    toplevel(env, root).ok_or_else(|| RulesError::Git("not inside a Git work tree".to_owned()))?;
  let root = top.as_path();
  if !base_resolves(env, root, base)? {
    return Ok(unavailable(&format!(
      "base ref {base} does not resolve to a commit"
    )));
  }
  let mut bundle = Bundle {
    available: true,
    files: Vec::new(),
    prompt_override: None,
    dropped: Vec::new(),
    notices: Vec::new(),
    bytes: 0,
  };
  for path in candidates(touched) {
    if !excluded(&path) {
      consider(env, root, base, &path, &mut bundle)?;
    }
  }
  Ok(bundle)
}

fn unavailable(reason: &str) -> Bundle {
  Bundle {
    available: false,
    files: Vec::new(),
    prompt_override: None,
    dropped: Vec::new(),
    notices: vec![format!("rules unavailable: {reason}")],
    bytes: 0,
  }
}

/// Root files, then the prompt override, then nested files nearest first.
fn candidates(touched: &[String]) -> Vec<String> {
  let mut ordered: Vec<String> = BASENAMES.iter().map(|name| (*name).to_owned()).collect();
  ordered.push(PROMPT_PATH.to_owned());
  ordered.extend(nested(touched));
  ordered
}

fn nested(touched: &[String]) -> Vec<String> {
  let mut dirs = BTreeSet::new();
  for path in touched {
    let mut dir = path.as_str();
    while let Some((parent, _)) = dir.rsplit_once('/') {
      dirs.insert(parent.to_owned());
      dir = parent;
    }
  }
  let mut found = Vec::new();
  for dir in &dirs {
    for name in BASENAMES {
      found.push(format!("{dir}/{name}"));
    }
  }
  found.sort_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));
  found
}

/// Directory depth of a candidate, from its separators.
fn depth(path: &str) -> usize {
  path.matches('/').count()
}

/// Paths the review never treats as conventions, checked before any read:
/// fixture trees, generated CLI docs, and this repo's plan/spec trees.
fn excluded(path: &str) -> bool {
  let parts: Vec<&str> = path.split('/').collect();
  let Some((_, dirs)) = parts.split_last() else {
    return false;
  };
  if dirs.contains(&"fixtures") {
    return true;
  }
  path.starts_with("docs/cli/")
    || [
      "docs/toolu/plans/",
      "docs/toolu/specs/",
      "docs/toolu/brainstorms/",
    ]
    .iter()
    .any(|tree| path.starts_with(tree))
}

/// Read one candidate into the bundle, or record why it stayed out.
fn consider(
  env: &Env,
  root: &Path,
  base: &str,
  path: &str,
  bundle: &mut Bundle,
) -> Result<(), RulesError> {
  let (bytes, truncated) = match read_blob(env, root, base, path)? {
    Blob::Absent => return Ok(()),
    Blob::Failed(reason) => {
      bundle.dropped.push(Omitted {
        path: path.to_owned(),
        reason,
      });
      return Ok(());
    }
    Blob::Present(bytes, truncated) => (bytes, truncated),
  };
  if truncated || bytes.len() > RULES_MAX_BYTES.saturating_sub(bundle.bytes) {
    bundle.dropped.push(Omitted {
      path: path.to_owned(),
      reason: CAP_REASON.to_owned(),
    });
    return Ok(());
  }
  let Ok(text) = String::from_utf8(bytes) else {
    bundle.dropped.push(Omitted {
      path: path.to_owned(),
      reason: "rule file is not UTF-8".to_owned(),
    });
    return Ok(());
  };
  bundle.bytes += text.len();
  let file = RuleFile {
    path: path.to_owned(),
    text,
  };
  if path == PROMPT_PATH {
    bundle.prompt_override = Some(file);
  } else {
    bundle.files.push(file);
  }
  Ok(())
}

/// One candidate's committed content.
enum Blob {
  /// No blob at the base for this path.
  Absent,
  /// Blob bytes and whether more was cut at the read limit.
  Present(Vec<u8>, bool),
  /// Git failed for this path; the reason is shown to people.
  Failed(String),
}

fn read_blob(env: &Env, root: &Path, base: &str, path: &str) -> Result<Blob, RulesError> {
  let object = format!("{base}:{path}");
  let out = run_git(env, root, &["cat-file", "blob", &object], READ_LIMIT)?;
  if out.timed_out {
    return Ok(Blob::Failed(format!(
      "git cat-file blob {object}: timed out"
    )));
  }
  if out.exit_code == 0 {
    return Ok(Blob::Present(out.stdout_bytes, out.truncated));
  }
  let stderr = out.stderr.trim();
  if stderr.contains("does not exist") {
    return Ok(Blob::Absent);
  }
  Ok(Blob::Failed(format!(
    "git cat-file blob {object}: {stderr}"
  )))
}

/// Whether `base` names a commit; exit 1 alone means it does not.
fn base_resolves(env: &Env, root: &Path, base: &str) -> Result<bool, RulesError> {
  let commit = format!("{base}^{{commit}}");
  let out = run_git(
    env,
    root,
    &["rev-parse", "--verify", "--quiet", &commit],
    4096,
  )?;
  if out.exit_code == 0 && !out.timed_out && !out.truncated {
    return Ok(true);
  }
  if out.exit_code == 1 && !out.timed_out {
    return Ok(false);
  }
  Err(RulesError::Git(format!(
    "git rev-parse {commit}: {}",
    out.stderr.trim()
  )))
}

/// Run one bounded Git command from the worktree top.
fn run_git(env: &Env, root: &Path, args: &[&str], max_bytes: usize) -> Result<Output, RulesError> {
  let mut spec = Spec::new(["git", "-C", &root.display().to_string()]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  spec.max_output_bytes = max_bytes;
  run(&spec).map_err(|err| RulesError::Git(format!("git {args:?}: {err:?}")))
}

#[cfg(test)]
#[path = "tests/rules_test.rs"]
mod tests;
