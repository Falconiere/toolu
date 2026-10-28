//! The committed review diff: base, noise, renames, and citable changed lines.

mod file;
pub(crate) mod git_io;
mod noise;
mod parse;
mod source;

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;
use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::base_branch;

use crate::budget::DIFF_CHUNK_MAX_BYTES;
use crate::contract::{Coverage, Omitted, Side};
use file::{FileDecision, inspect_file};
use git_io::{git, git_found, git_output, text};
use noise::status_noise;
use parse::{Status, names, statuses};
use source::batch_generated;

/// Status bytes read to tell whether the worktree has any change.
const STATUS_PROBE_BYTES: usize = 4096;

/// A failure that prevents the committed diff from being shaped safely.
#[derive(Debug, PartialEq, Eq)]
pub enum ShapeError {
  /// The chosen base is absent or cannot name a commit.
  BaseMissing(String),
  /// There is no common commit; shallow history may be the cause.
  NoMergeBase {
    /// Whether Git reports a shallow repository.
    shallow: bool,
  },
  /// The committed branch has no changed paths.
  EmptyDiff,
  /// Git failed, truncated output, or returned unusable metadata.
  Git(String),
}

/// The status of one fully shaped file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
  /// Newly added text.
  Added,
  /// Modified text.
  Modified,
  /// Deleted text; anchors use the old side.
  Deleted,
  /// One path moved to another, possibly with edits.
  Renamed,
  /// Type or other Git change.
  Other,
}

/// A line that changed in the committed diff and can anchor a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedLine {
  /// New for additions, old for removals.
  pub side: Side,
  /// One-based line in that side's committed file.
  pub line: u32,
  /// Text without the diff prefix.
  pub text: String,
}

/// One reviewed file; its patch has no context outside changed lines.
#[derive(Debug, Clone, Serialize)]
pub struct ShapedFile {
  /// Path on the new side, or the old path for a deletion.
  pub path: String,
  /// Prior path for a rename or deletion.
  pub old_path: Option<String>,
  /// Git's change kind.
  pub kind: FileKind,
  /// Zero-context patch for downstream evidence.
  pub patch: String,
  /// Only citable changed lines.
  pub lines: Vec<ChangedLine>,
}

/// A rename mapping for branch-local finding memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rename {
  /// Path before the move.
  pub old_path: String,
  /// Path after the move.
  pub path: String,
}

/// A whole-file evidence chunk below the fixed diff budget.
#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
  /// Indices into `Shaped::files`.
  pub files: Vec<usize>,
  /// Patch bytes in the chunk.
  pub bytes: usize,
}

/// A complete, gate-aligned committed diff for later review stages.
#[derive(Debug, Serialize)]
pub struct Shaped {
  /// Base ref resolved like the push-review gate.
  pub base_ref: String,
  /// The common commit behind base...HEAD.
  pub merge_base: String,
  /// The exact gate hash of the committed patch.
  pub diff_sha: String,
  /// Exact path set from the gate's name-only diff.
  pub gate_files: Vec<String>,
  /// Fully reviewed text files.
  pub files: Vec<ShapedFile>,
  /// Whole-file groupings under `DIFF_CHUNK_MAX_BYTES`.
  pub chunks: Vec<Chunk>,
  /// Reviewed, skipped, and unreviewed paths, partitioning `gate_files`.
  pub coverage: Coverage,
  /// Old-to-new rename map, including noisy files.
  pub renames: Vec<Rename>,
  /// Whether the worktree contains changes outside the committed diff.
  pub uncommitted: bool,
}

/// Shape the same committed diff the push gate hashes. Missing history, an
/// empty diff, or unreadable global metadata never becomes a pass.
///
/// # Errors
/// Returns `ShapeError` when Git cannot provide complete committed evidence.
/// Git 2.42 or newer is required for merge-base attributes in file patches.
pub fn shape(env: &Env, root: &Path, requested_base: Option<&str>) -> Result<Shaped, ShapeError> {
  // Pathspecs resolve against the working directory, so Git runs from the toplevel.
  let top =
    toplevel(env, root).ok_or_else(|| ShapeError::Git("not inside a Git work tree".to_owned()))?;
  let root = top.as_path();
  let (mut shaped, statuses) = start(env, root, requested_base)?;
  populate(env, root, statuses, &mut shaped)?;
  // A commit or base move mid-run would pair the hash with other evidence; patches
  // use the merge-base resolved before the first hash, so it must still hold too.
  let hash = diff_sha(env, root, &shaped.base_ref)
    .ok_or_else(|| ShapeError::Git("gate diff hash failed".to_owned()))?;
  if hash != shaped.diff_sha
    || resolve_merge_base(env, root, &shaped.base_ref)? != shaped.merge_base
  {
    return Err(ShapeError::Git(
      "committed diff changed while shaping".to_owned(),
    ));
  }
  Ok(shaped)
}

fn start(
  env: &Env,
  root: &Path,
  requested_base: Option<&str>,
) -> Result<(Shaped, Vec<Status>), ShapeError> {
  let (base_ref, merge_base) = resolve_base(env, root, requested_base)?;
  let diff_sha = diff_sha(env, root, &base_ref)
    .ok_or_else(|| ShapeError::Git("gate diff hash failed".to_owned()))?;
  let range = format!("{base_ref}...HEAD");
  let gate_files = names(&git(env, root, &["diff", &range, "--name-only", "-z"])?)?;
  if gate_files.is_empty() {
    return Err(ShapeError::EmptyDiff);
  }
  let metadata = git(env, root, &["diff", "-M", "--name-status", "-z", &range])?;
  let statuses = statuses(&metadata)?;
  // Only emptiness matters, so a dropped tail still means uncommitted changes.
  let (status, truncated) = git_output(
    env,
    root,
    &[
      "--no-optional-locks",
      "status",
      "--porcelain=v1",
      "-z",
      "--untracked-files=normal",
    ],
    STATUS_PROBE_BYTES,
  )?;
  let uncommitted = truncated || !status.is_empty();
  let shaped = Shaped {
    base_ref,
    merge_base,
    diff_sha,
    gate_files,
    files: Vec::new(),
    chunks: Vec::new(),
    coverage: Coverage::default(),
    renames: Vec::new(),
    uncommitted,
  };
  Ok((shaped, statuses))
}

fn resolve_base(
  env: &Env,
  root: &Path,
  requested_base: Option<&str>,
) -> Result<(String, String), ShapeError> {
  let base_ref = requested_base
    .or_else(|| env.get("PUSH_REVIEW_BASE"))
    .map_or_else(|| base_branch(env, Some(root), root), str::to_owned);
  if base_ref.starts_with('-') || base_ref.is_empty() {
    return Err(ShapeError::BaseMissing(base_ref));
  }
  // `--verify --quiet` exits 1 only for a name that is not a commit.
  let commit = format!("{base_ref}^{{commit}}");
  if git_found(env, root, &["rev-parse", "--verify", "--quiet", &commit])?.is_none() {
    return Err(ShapeError::BaseMissing(base_ref));
  }
  let merge_base = resolve_merge_base(env, root, &base_ref)?;
  Ok((base_ref, merge_base))
}

fn resolve_merge_base(env: &Env, root: &Path, base_ref: &str) -> Result<String, ShapeError> {
  let Some(out) = git_found(env, root, &["merge-base", base_ref, "HEAD"])? else {
    let shallow = git(env, root, &["rev-parse", "--is-shallow-repository"])
      .ok()
      .and_then(|bytes| text(bytes).ok())
      .is_some_and(|text| text.trim() == "true");
    return Err(ShapeError::NoMergeBase { shallow });
  };
  let commit = text(out)?.trim().to_owned();
  if commit.is_empty() {
    return Err(ShapeError::Git(
      "git merge-base returned no commit".to_owned(),
    ));
  }
  Ok(commit)
}

/// Shared committed inputs for one file's noise and patch inspection.
struct FileContext<'a> {
  env: &'a Env,
  root: &'a Path,
  range: &'a str,
  merge_base: &'a str,
  generated: Option<&'a BTreeSet<String>>,
}

fn populate(
  env: &Env,
  root: &Path,
  statuses: Vec<Status>,
  shaped: &mut Shaped,
) -> Result<(), ShapeError> {
  let gate_set: BTreeSet<String> = shaped.gate_files.iter().cloned().collect();
  // Path noise is skipped without reading blobs, so large lockfiles are never grepped.
  let candidates: Vec<&Status> = statuses
    .iter()
    .filter(|status| status_noise(status).is_none())
    .collect();
  let generated = batch_generated(env, root, &candidates, &shaped.merge_base);
  let merge_base = shaped.merge_base.clone();
  let patch_range = format!("{merge_base}..HEAD");
  let context = FileContext {
    env,
    root,
    range: &patch_range,
    merge_base: &merge_base,
    generated: generated.as_ref(),
  };
  let mut seen = BTreeSet::new();
  for status in statuses {
    let covered = register_status(&status, &gate_set, &mut seen, &mut shaped.renames)?;
    shape_file(&context, &status, &covered, shaped);
  }
  for path in &shaped.gate_files {
    if !seen.contains(path) {
      shaped.coverage.unreviewed.push(Omitted {
        path: path.clone(),
        reason: "diff metadata did not account for this gate path".to_owned(),
      });
    }
  }
  let coverage = &mut shaped.coverage;
  coverage.reviewed.sort();
  coverage.skipped.sort_by(|a, b| a.path.cmp(&b.path));
  coverage.unreviewed.sort_by(|a, b| a.path.cmp(&b.path));
  Ok(())
}

fn register_status(
  status: &Status,
  gate_set: &BTreeSet<String>,
  seen: &mut BTreeSet<String>,
  renames: &mut Vec<Rename>,
) -> Result<Vec<String>, ShapeError> {
  if !gate_set.contains(status.path.as_str()) || !seen.insert(status.path.clone()) {
    return Err(ShapeError::Git(format!(
      "diff metadata disagrees with gate paths: {}",
      status.path
    )));
  }
  let mut covered = vec![status.path.clone()];
  if status.kind == FileKind::Renamed
    && let Some(old_path) = &status.old_path
  {
    renames.push(Rename {
      old_path: old_path.clone(),
      path: status.path.clone(),
    });
    if old_path != &status.path && gate_set.contains(old_path) {
      if !seen.insert(old_path.clone()) {
        return Err(ShapeError::Git(format!(
          "diff metadata repeats gate path: {old_path}"
        )));
      }
      covered.push(old_path.clone());
    }
  }
  Ok(covered)
}

fn shape_file(context: &FileContext<'_>, status: &Status, covered: &[String], shaped: &mut Shaped) {
  if let Some(reason) = status_noise(status) {
    omit(&mut shaped.coverage.skipped, covered, reason);
    return;
  }
  match inspect_file(context, status) {
    Ok(FileDecision::Skipped(reason)) => omit(&mut shaped.coverage.skipped, covered, reason),
    Ok(FileDecision::Unreviewed(reason)) => omit(&mut shaped.coverage.unreviewed, covered, reason),
    Ok(FileDecision::Reviewed(file)) => {
      let bytes = file.patch.len();
      let index = shaped.files.len();
      shaped.files.push(file);
      shaped.coverage.reviewed.extend_from_slice(covered);
      if shaped
        .chunks
        .last()
        .is_none_or(|chunk| chunk.bytes + bytes > DIFF_CHUNK_MAX_BYTES)
      {
        shaped.chunks.push(Chunk {
          files: Vec::new(),
          bytes: 0,
        });
      }
      if let Some(chunk) = shaped.chunks.last_mut() {
        chunk.files.push(index);
        chunk.bytes += bytes;
      }
    }
    // The reason is read by people: Git's message, not Rust's Debug form.
    Err(ShapeError::Git(detail)) => omit(&mut shaped.coverage.unreviewed, covered, &detail),
    Err(err) => omit(
      &mut shaped.coverage.unreviewed,
      covered,
      &format!("{err:?}"),
    ),
  }
}

fn omit(omitted: &mut Vec<Omitted>, paths: &[String], reason: &str) {
  omitted.extend(paths.iter().map(|path| Omitted {
    path: path.clone(),
    reason: reason.to_owned(),
  }));
}

#[cfg(test)]
#[path = "tests/shape_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/shape_edge_test.rs"]
mod edge_tests;
