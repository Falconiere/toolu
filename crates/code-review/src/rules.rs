//! Repository conventions from the base ref, within `budget::RULES_MAX_BYTES`.
//! The caller-supplied base (the shape stage's merge-base) is resolved once to
//! a commit id; candidates are listed with `git ls-tree` at that commit and read
//! by blob id with `git cat-file`, never from HEAD or the working tree, so a
//! branch cannot soften its own review. Rule text is reference data for the
//! prompt builder (#666); nothing here feeds a verdict.

use std::collections::{BTreeMap, BTreeSet};
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
/// Why a symlinked candidate was left out: its blob is a link path, not rules.
const LINK_REASON: &str = "rule file is a symlink";
/// One byte past the cap, so an over-cap blob is known without reading it all.
const READ_LIMIT: usize = RULES_MAX_BYTES + 1;
/// Candidates per `ls-tree` call, so a wide diff stays inside argv limits.
const LIST_CHUNK: usize = 256;
/// Output bound of one `ls-tree` listing of at most `LIST_CHUNK` entries.
const LIST_MAX_BYTES: usize = 1024 * 1024;
/// Generated CLI docs and this repo's plan/spec trees; never conventions.
const EXCLUDED_TREES: [&str; 4] = [
  "docs/cli/",
  "docs/toolu/plans/",
  "docs/toolu/specs/",
  "docs/toolu/brainstorms/",
];

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
  let Some(commit) = resolve(env, root, base)? else {
    return Ok(unavailable(&format!(
      "base ref {base} does not resolve to a commit"
    )));
  };
  let paths: Vec<String> = candidates(touched)
    .into_iter()
    .filter(|path| !excluded(path))
    .collect();
  let entries = list(env, root, &commit, &paths)?;
  let mut bundle = Bundle {
    available: true,
    files: Vec::new(),
    prompt_override: None,
    dropped: Vec::new(),
    notices: Vec::new(),
    bytes: 0,
  };
  for path in &paths {
    if let Some(entry) = entries.get(path) {
      consider(env, root, path, entry, &mut bundle)?;
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
/// any `fixtures` directory, generated CLI docs, and plan/spec trees.
fn excluded(path: &str) -> bool {
  let dirs = path.rsplit_once('/').map_or("", |(dirs, _)| dirs);
  dirs.split('/').any(|dir| dir == "fixtures")
    || EXCLUDED_TREES.iter().any(|tree| path.starts_with(tree))
}

/// One candidate's blob entry in the base commit's tree.
struct Entry {
  /// Blob id, read by id so the base cannot move between reads.
  oid: String,
  /// Blob size, so an over-budget file is dropped without being read.
  size: usize,
  /// Mode 120000: the blob holds a link path, not rule text.
  link: bool,
}

/// Blob entries among `paths` at `commit`. An absent path is simply not listed,
/// so absence never depends on Git's (translatable) messages or on whether the
/// path exists in the working tree.
fn list(
  env: &Env,
  root: &Path,
  commit: &str,
  paths: &[String],
) -> Result<BTreeMap<String, Entry>, RulesError> {
  let mut entries = BTreeMap::new();
  for chunk in paths.chunks(LIST_CHUNK) {
    let mut args = vec!["--literal-pathspecs", "ls-tree", "-z", "-l", commit, "--"];
    args.extend(chunk.iter().map(String::as_str));
    let out = run_git(env, root, &args, LIST_MAX_BYTES)?;
    if out.exit_code != 0 || out.timed_out || out.truncated {
      return Err(RulesError::Git(format!(
        "git ls-tree {commit}: exit {}, truncated={}, timed_out={}: {}",
        out.exit_code,
        out.truncated,
        out.timed_out,
        out.stderr.trim()
      )));
    }
    entries.extend(parse_listing(&out.stdout_bytes)?);
  }
  Ok(entries)
}

/// The blob records of one `ls-tree -z -l` listing (`mode type oid size\tpath`);
/// trees and submodules named like a rule file are not files and are skipped.
fn parse_listing(stdout: &[u8]) -> Result<Vec<(String, Entry)>, RulesError> {
  let mut blobs = Vec::new();
  for record in stdout
    .split(|byte| *byte == 0)
    .filter(|record| !record.is_empty())
  {
    let text = String::from_utf8_lossy(record);
    let bad = || RulesError::Git(format!("unusable git ls-tree record {text:?}"));
    let (meta, path) = text.split_once('\t').ok_or_else(bad)?;
    let fields: Vec<&str> = meta.split_whitespace().collect();
    let [mode, kind, oid, size] = fields.as_slice() else {
      return Err(bad());
    };
    if *kind == "blob" {
      let size: usize = size.parse().ok().ok_or_else(bad)?;
      let entry = Entry {
        oid: (*oid).to_owned(),
        size,
        link: *mode == "120000",
      };
      blobs.push((path.to_owned(), entry));
    }
  }
  Ok(blobs)
}

/// Read one listed candidate into the bundle, or record why it stayed out.
fn consider(
  env: &Env,
  root: &Path,
  path: &str,
  entry: &Entry,
  bundle: &mut Bundle,
) -> Result<(), RulesError> {
  match admit(
    env,
    root,
    entry,
    RULES_MAX_BYTES.saturating_sub(bundle.bytes),
  )? {
    Ok(text) => {
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
    }
    Err(reason) => bundle.dropped.push(Omitted {
      path: path.to_owned(),
      reason,
    }),
  }
  Ok(())
}

/// The entry's text when it fits `remaining`, else why it stays out. A Git
/// failure on this blob is such a reason, so one bad file cannot hide the rest.
fn admit(
  env: &Env,
  root: &Path,
  entry: &Entry,
  remaining: usize,
) -> Result<Result<String, String>, RulesError> {
  if entry.link {
    return Ok(Err(LINK_REASON.to_owned()));
  }
  if entry.size > remaining {
    return Ok(Err(CAP_REASON.to_owned()));
  }
  let out = run_git(env, root, &["cat-file", "blob", &entry.oid], READ_LIMIT)?;
  if out.timed_out || out.exit_code != 0 {
    let detail = if out.timed_out {
      "timed out"
    } else {
      out.stderr.lines().next().unwrap_or_default().trim()
    };
    return Ok(Err(format!("git cat-file blob {}: {detail}", entry.oid)));
  }
  if out.truncated || out.stdout_bytes.len() > remaining {
    return Ok(Err(CAP_REASON.to_owned()));
  }
  Ok(
    String::from_utf8(out.stdout_bytes)
      .ok()
      .ok_or_else(|| "rule file is not UTF-8".to_owned()),
  )
}

/// The commit id `base` names, or `None` when it names none (exit 1 alone).
fn resolve(env: &Env, root: &Path, base: &str) -> Result<Option<String>, RulesError> {
  let commit = format!("{base}^{{commit}}");
  let out = run_git(
    env,
    root,
    &["rev-parse", "--verify", "--quiet", &commit],
    4096,
  )?;
  let oid = out.stdout.trim();
  if out.exit_code == 0 && !out.timed_out && !out.truncated && !oid.is_empty() {
    return Ok(Some(oid.to_owned()));
  }
  if out.exit_code == 1 && !out.timed_out {
    return Ok(None);
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
