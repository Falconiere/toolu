//! The provenance note: JSON under `refs/notes/git-better` with what a squash
//! merge or a rebase loses (the branch, base, host and session the commit was
//! made on) and the areas and files it changed. Trailers stay the source of
//! truth: readers take areas from the diff and only the provenance from a note,
//! so a missing note loses only the provenance.

use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::areas::summarize;
use crate::gitio::{FileChange, changes, git};

/// The notes ref this plugin writes.
pub(crate) const NOTES_REF: &str = "refs/notes/git-better";
/// The note's schema name.
pub(crate) const SCHEMA: &str = "git-better/v1";
/// The most files a note lists.
pub(crate) const MAX_FILES: usize = 200;

/// Where a commit was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Provenance {
  /// The branch it was committed on.
  pub(crate) branch: String,
  /// The base branch at the time.
  pub(crate) base: String,
  /// The agent host, such as `claude`.
  pub(crate) host: String,
  /// The host's session id.
  pub(crate) session: String,
}

fn file(change: &FileChange) -> Value {
  json!({
    "path": change.path,
    "oldPath": change.old_path,
    "status": change.status,
    "added": change.added,
    "deleted": change.deleted,
  })
}

/// The note for a commit with `changes`, made where `provenance` says.
pub(crate) fn body(provenance: &Provenance, changes: &[FileChange], roots: &[String]) -> Value {
  let areas: Vec<Value> = summarize(changes, roots)
    .iter()
    .map(|area| {
      json!({
        "area": area.name,
        "files": area.files,
        "added": area.added,
        "deleted": area.deleted,
      })
    })
    .collect();
  let files: Vec<Value> = changes.iter().take(MAX_FILES).map(file).collect();
  json!({
    "schema": SCHEMA,
    "branch": provenance.branch,
    "base": provenance.base,
    "host": provenance.host,
    "session": provenance.session,
    "areas": areas,
    "files": files,
    "truncated": changes.len() > MAX_FILES,
  })
}

/// Attach the note to `commit` unless it already has one; whether one was written.
///
/// # Errors
/// When git cannot read the commit's changes or store the note.
pub(crate) fn write(
  env: &Env,
  dir: &Path,
  commit: &str,
  provenance: &Provenance,
  roots: &[String],
) -> Result<bool, String> {
  let shown = git(
    env,
    dir,
    &["notes", "--ref", NOTES_REF, "list", commit],
    &[],
  );
  if shown.is_ok_and(|listed| !listed.trim().is_empty()) {
    return Ok(false);
  }
  let found = changes(env, dir, commit)?;
  let text = body(provenance, &found, roots).to_string();
  let args = ["notes", "--ref", NOTES_REF, "add", "-F", "-", commit];
  git(
    &as_committer(env, dir, commit)?,
    dir,
    &args,
    text.as_bytes(),
  )?;
  Ok(true)
}

/// `env` with `commit`'s committer as the note's author and committer: storing a
/// note makes a commit on the notes ref, and the hook's own environment may have
/// no identity even though the agent's commit had one.
fn as_committer(env: &Env, dir: &Path, commit: &str) -> Result<Env, String> {
  let who = git(env, dir, &["log", "-1", "--format=%cn%x1f%ce", commit], &[])?;
  let (name, email) = who
    .trim_end()
    .split_once('\x1f')
    .ok_or_else(|| format!("no committer on {commit}"))?;
  Ok(
    env
      .clone()
      .with("GIT_AUTHOR_NAME", name)
      .with("GIT_AUTHOR_EMAIL", email)
      .with("GIT_COMMITTER_NAME", name)
      .with("GIT_COMMITTER_EMAIL", email),
  )
}

#[cfg(test)]
#[path = "tests/note_test.rs"]
mod tests;
