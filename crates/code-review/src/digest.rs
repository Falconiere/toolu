//! `toolu code-review digest`: the branch and local changes by file and item,
//! with no model call. It reads the same snapshot `run` reviews (`shape::review`):
//! per file its line counts, the outline items its changes sit in, the names its
//! removed lines defined and a few bounded hunks; noise files are listed as skipped.

mod hunks;
mod render;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use clap::ArgMatches;
use regex::Regex;
use serde::Serialize;
use toolu_engine::outline::{FileOutline, enclosing, outline, supported};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_runtime::host::roots::Roots;

use crate::contract::{Omitted, Side};
use crate::pipeline::{project_root, scope};
use crate::review::context::DEFINITION;
use crate::review::window::centers;
use crate::shape::{self, FileKind, ShapeError, Shaped, ShapedFile};
use hunks::Hunk;

/// The most files printed.
const MAX_FILES: usize = 80;
/// The most items named per file.
const MAX_ITEMS: usize = 10;
/// The reason `scope` gives a reviewed path outside `--path`.
const OUTSIDE: &str = "outside requested paths";

/// One digest: the printed files and the paths left out of them.
#[derive(Debug, Serialize)]
pub(crate) struct Digest {
  base: String,
  merge_base: String,
  uncommitted: bool,
  added: usize,
  removed: usize,
  files: Vec<FileDigest>,
  files_omitted: usize,
  /// Files printed without hunks once the hunk budget ran out.
  hunks_budget_files: usize,
  /// Why no file names its items, when the outline failed.
  items_unavailable: Option<String>,
  skipped: Vec<Omitted>,
  unread: Vec<Omitted>,
}

/// One changed file.
#[derive(Debug, Serialize)]
struct FileDigest {
  path: String,
  old_path: Option<String>,
  kind: FileKind,
  added: usize,
  removed: usize,
  items: Vec<ItemRef>,
  items_omitted: usize,
  removed_items: Vec<String>,
  hunks: Vec<Hunk>,
  hunks_omitted: usize,
}

/// An item a change sits in; `new` when every line of it was added.
#[derive(Debug, Serialize)]
struct ItemRef {
  name: String,
  start: usize,
  end: usize,
  new: bool,
}

/// Run `toolu code-review digest`.
pub(crate) fn run(env: &Env, ctx: &Ctx, verb: &ArgMatches) -> Outcome {
  match digest(env, ctx, verb) {
    Ok(digest) if ctx.json => match serde_json::to_string(&digest) {
      Ok(text) => Outcome::data(text),
      Err(err) => Outcome::failed(Exit::Failure, format!("code-review digest: {err}")),
    },
    Ok(digest) => Outcome::data(render::text(&digest)),
    Err(message) => Outcome::failed(Exit::Failure, format!("code-review digest: {message}")),
  }
}

fn digest(env: &Env, ctx: &Ctx, verb: &ArgMatches) -> Result<Digest, String> {
  let roots = Roots::new(env.clone(), ctx.host);
  let root = project_root(&roots)?;
  let top = toplevel(env, &root).ok_or("not inside a Git work tree")?;
  let state_root = roots
    .project_state_root(None, Some(&top))
    .ok_or("state root unavailable")?;
  let since = verb.get_one::<String>("since").map(String::as_str);
  let mut shaped = match shape::review(env, &top, since, &state_root) {
    Ok(shaped) => shaped,
    Err(ShapeError::EmptyDiff) => {
      let (base, merge_base) = shape::resolve_base(env, &top, since).map_err(describe)?;
      return Ok(empty(base, merge_base));
    }
    Err(err) => return Err(describe(err)),
  };
  if let Some(paths) = verb.get_many::<String>("path") {
    let paths: Vec<&str> = paths.map(String::as_str).collect();
    scope(&mut shaped, &paths);
    // `scope` narrows only reviewed paths; skipped and unread ones are narrowed here.
    let coverage = &mut shaped.coverage;
    coverage
      .skipped
      .retain(|one| paths.contains(&one.path.as_str()));
    coverage
      .unreviewed
      .retain(|one| one.reason != OUTSIDE && paths.contains(&one.path.as_str()));
  }
  Ok(build(env, &top, &shaped))
}

fn describe(err: ShapeError) -> String {
  match err {
    ShapeError::BaseMissing(base) => format!("base not found: {base}"),
    ShapeError::NoMergeBase { shallow: true } => {
      "no merge base with the base; the repository is shallow".to_owned()
    }
    ShapeError::NoMergeBase { shallow: false } => "no merge base with the base".to_owned(),
    ShapeError::EmptyDiff => "no changes".to_owned(),
    ShapeError::Git(message) => message,
  }
}

fn empty(base: String, merge_base: String) -> Digest {
  Digest {
    base,
    merge_base,
    uncommitted: false,
    added: 0,
    removed: 0,
    files: Vec::new(),
    files_omitted: 0,
    hunks_budget_files: 0,
    items_unavailable: None,
    skipped: Vec::new(),
    unread: Vec::new(),
  }
}

/// The capped digest of `shaped`, whose paths are relative to `top`.
pub(crate) fn build(env: &Env, top: &Path, shaped: &Shaped) -> Digest {
  let reviewed: BTreeSet<&str> = shaped
    .coverage
    .reviewed
    .iter()
    .map(String::as_str)
    .collect();
  let files: Vec<&ShapedFile> = shaped
    .files
    .iter()
    .filter(|file| reviewed.contains(file.path.as_str()))
    .collect();
  let mut digest = empty(shaped.base_ref.clone(), shaped.merge_base.clone());
  digest.uncommitted = shaped.uncommitted;
  digest.skipped.clone_from(&shaped.coverage.skipped);
  digest.unread.clone_from(&shaped.coverage.unreviewed);
  for file in &files {
    let (added, removed) = counts(file);
    digest.added += added;
    digest.removed += removed;
  }
  digest.files_omitted = files.len().saturating_sub(MAX_FILES);
  let shown = files.get(..MAX_FILES).unwrap_or(&files);
  let outlines = outlines(env, top, shown);
  if let Err(reason) = &outlines {
    digest.items_unavailable = Some(reason.clone());
  }
  let mut budget = hunks::Budget::default();
  for file in shown {
    let outline = outlines
      .as_ref()
      .ok()
      .and_then(|found| found.iter().find(|one| one.path == file.path));
    digest.files.push(file_digest(file, outline, &mut budget));
  }
  digest.hunks_budget_files = budget.dropped;
  digest
}

fn file_digest(
  file: &ShapedFile,
  outline: Option<&FileOutline>,
  budget: &mut hunks::Budget,
) -> FileDigest {
  let (added, removed) = counts(file);
  let mut items = outline
    .map(|outline| items(file, outline))
    .unwrap_or_default();
  let items_omitted = items.len().saturating_sub(MAX_ITEMS);
  items.truncate(MAX_ITEMS);
  let (hunks, hunks_omitted) = budget.take(&file.patch);
  FileDigest {
    path: file.path.clone(),
    old_path: file
      .old_path
      .clone()
      .filter(|_| file.kind == FileKind::Renamed),
    kind: file.kind,
    added,
    removed,
    items,
    items_omitted,
    removed_items: removed_names(file, outline),
    hunks,
    hunks_omitted,
  }
}

fn counts(file: &ShapedFile) -> (usize, usize) {
  let added = file
    .lines
    .iter()
    .filter(|line| line.side == Side::New)
    .count();
  (added, file.lines.len() - added)
}

/// Outlines of the files whose new side exists in a language `outline` reads.
fn outlines(env: &Env, top: &Path, files: &[&ShapedFile]) -> Result<Vec<FileOutline>, String> {
  let paths: Vec<String> = files
    .iter()
    .filter(|file| file.kind != FileKind::Deleted && supported(&file.path))
    .filter(|file| top.join(&file.path).is_file())
    .map(|file| file.path.clone())
    .collect();
  outline(env, top, &paths)
}

/// The innermost item at each change, in file order.
fn items(file: &ShapedFile, outline: &FileOutline) -> Vec<ItemRef> {
  let added: BTreeSet<usize> = file
    .lines
    .iter()
    .filter(|line| line.side == Side::New)
    .filter_map(|line| usize::try_from(line.line).ok())
    .collect();
  let mut found = BTreeMap::new();
  for center in centers(file) {
    if let Some(item) = enclosing(outline, center) {
      let new = (item.start..=item.end).all(|line| added.contains(&line));
      found.insert((item.start, item.end, item.name.clone()), new);
    }
  }
  found
    .into_iter()
    .map(|((start, end, name), new)| ItemRef {
      name,
      start,
      end,
      new,
    })
    .collect()
}

/// Names the removed lines defined that the new side no longer has.
fn removed_names(file: &ShapedFile, outline: Option<&FileOutline>) -> Vec<String> {
  let Ok(pattern) = Regex::new(DEFINITION) else {
    return Vec::new();
  };
  let kept: BTreeSet<&str> = outline
    .map(|outline| {
      outline
        .items
        .iter()
        .map(|item| item.name.as_str())
        .collect()
    })
    .unwrap_or_default();
  let mut names: Vec<String> = Vec::new();
  for line in file.lines.iter().filter(|line| line.side == Side::Old) {
    for found in pattern.captures_iter(&line.text) {
      if let Some(name) = found.get(1).map(|name| name.as_str())
        && !kept.contains(name)
        && !names.iter().any(|seen| seen == name)
      {
        names.push(name.to_owned());
      }
    }
  }
  names
}

#[cfg(test)]
#[path = "tests/digest_test.rs"]
mod tests;
