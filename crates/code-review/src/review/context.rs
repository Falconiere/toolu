//! Caller context for one changed file: the definitions its changes sit in, and
//! every other `HEAD` line that names them, so the model sees who depends on what
//! changed even when those files are not in the diff. Definitions are found by a
//! language-agnostic keyword pattern and references by `git grep -w` over the
//! committed `HEAD` tree, never the worktree. A name with too many hits says too
//! little to be worth its bytes and is skipped with a note.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;
use toolu_runtime::env::Env;

use super::window::centers;
use crate::budget::CONTEXT_HITS_MAX;
use crate::contract::Side;
use crate::shape::git_io::git_process;
use crate::shape::{Shaped, ShapedFile};

/// A definition keyword followed by the name it defines.
const DEFINITION: &str =
  r"\b(?:fn|def|function|func|class|struct|enum|trait|interface|type)\s+([A-Za-z_][A-Za-z0-9_]*)";
/// The most `git grep` output read for one name; more is too many hits anyway.
const GREP_MAX_BYTES: usize = 64 * 1024;

/// One `HEAD` line naming a definition: `(path, line, text)`.
type Hit = (String, u32, String);

/// The state of one review's caller search: the definition pattern compiled
/// once, and each name's raw `HEAD` hits (`None` when over the limit or
/// unreadable) so a name enclosing changes in several files is searched once.
pub(super) struct Callers {
  pattern: Result<Regex, String>,
  hits: BTreeMap<String, Option<Vec<Hit>>>,
  /// The most caller bytes one file may take.
  cap: usize,
  /// How many `git grep` processes this review has run.
  pub(super) searches: usize,
}

impl Callers {
  pub(super) fn new(cap: usize) -> Self {
    Self {
      pattern: Regex::new(DEFINITION).map_err(|err| err.to_string()),
      hits: BTreeMap::new(),
      cap,
      searches: 0,
    }
  }

  /// The references to the definitions `file`'s changes sit in, as `name:` blocks
  /// of `path:line: text` lines within `cap` bytes, or `None` when there are none.
  pub(super) fn of(
    &mut self,
    (env, root): (&Env, &Path),
    shaped: &Shaped,
    (file, text): (&ShapedFile, &str),
    notes: &mut Vec<String>,
  ) -> Option<String> {
    let cap = self.cap;
    let pattern = match &self.pattern {
      Ok(pattern) => pattern,
      Err(err) => {
        notes.push(format!("caller context unavailable: {err}"));
        return None;
      }
    };
    let names = enclosing(pattern, file, text);
    let changed = changed_lines(shaped);
    let mut out = String::new();
    for (name, defined_at) in names {
      let cached = self.hits.entry(name.clone()).or_insert_with(|| {
        self.searches += 1;
        references(env, root, &name)
      });
      let Some(hits) = cached else {
        notes.push(format!(
          "callers of {name} left out: over {CONTEXT_HITS_MAX} references or unreadable"
        ));
        continue;
      };
      let lines = unseen(hits, (&file.path, defined_at), &changed);
      if lines.is_empty() {
        continue;
      }
      let block = format!("{name}:\n{}", lines.concat());
      if out.len() + block.len() > cap {
        notes.push(format!(
          "callers of {name} left out: context over {cap} bytes"
        ));
        continue;
      }
      out.push_str(&block);
    }
    (!out.is_empty()).then_some(out)
  }
}

/// The new-side `(path, line)` positions the diff changes.
fn changed_lines(shaped: &Shaped) -> BTreeSet<(&str, u32)> {
  shaped
    .files
    .iter()
    .flat_map(|file| file.lines.iter().map(move |line| (file, line)))
    .filter(|(_, line)| line.side == Side::New)
    .map(|(file, line)| (file.path.as_str(), line.line))
    .collect()
}

/// `hits` as `path:line: text` lines, less the definition at `(path, line)` and
/// every changed line; the cached `hits` themselves stay raw for other files.
fn unseen(
  hits: &[Hit],
  (own, defined_at): (&str, u32),
  changed: &BTreeSet<(&str, u32)>,
) -> Vec<String> {
  hits
    .iter()
    .filter(|(path, line, _)| {
      (path != own || *line != defined_at) && !changed.contains(&(path.as_str(), *line))
    })
    .map(|(path, line, text)| format!("{path}:{line}: {text}\n"))
    .collect()
}

/// Each name defined at or above a change in `text`, with its defining line.
fn enclosing(pattern: &Regex, file: &ShapedFile, text: &str) -> BTreeMap<String, u32> {
  let definitions: Vec<(usize, &str)> = text
    .lines()
    .enumerate()
    .filter_map(|(index, line)| {
      let name = pattern.captures(line)?.get(1)?.as_str();
      Some((index + 1, name))
    })
    .collect();
  centers(file)
    .into_iter()
    .filter_map(|center| {
      let before = definitions.partition_point(|(line, _)| *line <= center);
      let (line, name) = definitions.get(before.checked_sub(1)?)?;
      Some(((*name).to_owned(), u32::try_from(*line).ok()?))
    })
    .collect()
}

/// Every `HEAD` line naming `name` as a whole word, as `(path, line, text)`;
/// `None` when Git fails or there are more than `CONTEXT_HITS_MAX`.
fn references(env: &Env, root: &Path, name: &str) -> Option<Vec<Hit>> {
  // `-z` ends the path and the line number with NUL, so a path may hold `:`.
  let args = [
    "grep", "-n", "-z", "-w", "-I", "-F", "-e", name, "HEAD", "--",
  ];
  let out = git_process(env, root, &args, GREP_MAX_BYTES).ok()?;
  // Exit 1 is Git's "no match": an empty answer, not a failure.
  if out.timed_out || out.truncated || !matches!(out.exit_code, 0 | 1) {
    return None;
  }
  let hits: Vec<Hit> = String::from_utf8_lossy(&out.stdout_bytes)
    .lines()
    .filter_map(|hit| {
      let mut parts = hit.strip_prefix("HEAD:")?.splitn(3, '\0');
      let (path, line, text) = (parts.next()?, parts.next()?, parts.next()?);
      Some((
        path.to_owned(),
        line.parse().ok()?,
        text.trim_end().to_owned(),
      ))
    })
    .collect();
  (hits.len() <= CONTEXT_HITS_MAX).then_some(hits)
}

#[cfg(test)]
#[path = "tests/context_test.rs"]
mod tests;
