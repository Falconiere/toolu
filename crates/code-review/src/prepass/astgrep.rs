//! The structural scanner: the engine's `scan_inline` over the HEAD blobs of the
//! shaped files, so the review sees committed text, not the worktree. Hits count
//! only on lines the diff added or changed.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use serde_json::Map;
use tempfile::TempDir;
use toolu_engine::quality::{AstGrepHit, AstGrepScan, EditedFile, ScanFailure};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_runtime::process::{Spec, run};
use toolu_runtime::registry::rule::RuleContext;

use super::secrets::redact_line;
use super::{Notice, NoticeKind, Scan, finding_id};
use crate::budget::BLOB_MAX_BYTES;
use crate::contract::{Finding, Severity, Side, Source};
use crate::shape::Shaped;

/// The embedded review ruleset; each document carries its own `message`.
const RULES: &str = include_str!("rules.yaml");

fn notice(kind: NoticeKind, text: String) -> Notice {
  Notice { kind, text }
}

/// The `message:` of rule `id` in the embedded set.
fn message(id: &str) -> String {
  let wanted = format!("id: {id}");
  RULES
    .split("\n---\n")
    .find(|doc| doc.lines().any(|line| line == wanted))
    .and_then(|doc| doc.lines().find_map(|line| line.strip_prefix("message: ")))
    .map_or_else(|| format!("structural rule {id} matched"), str::to_owned)
}

/// Write the HEAD blob of `path` under `temp` at the same relative path.
fn materialize(env: &Env, top: &Path, temp: &Path, path: &str) -> Result<EditedFile, String> {
  if path.starts_with('-') {
    return Err("path starts with '-' and could be read as a flag".to_owned());
  }
  let rel = Path::new(path);
  if rel.is_absolute()
    || rel
      .components()
      .any(|part| !matches!(part, Component::Normal(_)))
  {
    return Err("path leaves the repository".to_owned());
  }
  let mut spec = Spec::new(["git", "-C", &top.display().to_string(), "cat-file", "blob"]);
  spec.argv.push(format!("HEAD:{path}"));
  spec.env = Some(env.clone());
  spec.max_output_bytes = BLOB_MAX_BYTES;
  let out = run(&spec).map_err(|err| format!("git cat-file: {err:?}"))?;
  if out.timed_out {
    return Err("git cat-file: timed out".to_owned());
  }
  if out.truncated {
    return Err(format!("git cat-file: blob exceeds {BLOB_MAX_BYTES} bytes"));
  }
  if out.exit_code != 0 {
    return Err(format!(
      "git cat-file: exit {}: {}",
      out.exit_code,
      out.stderr.trim()
    ));
  }
  std::str::from_utf8(&out.stdout_bytes).map_err(|err| format!("not UTF-8: {err}"))?;
  let absolute = temp.join(rel);
  if let Some(parent) = absolute.parent() {
    fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
  }
  fs::write(&absolute, &out.stdout_bytes)
    .map_err(|err| format!("{}: {err}", absolute.display()))?;
  Ok(EditedFile {
    path: path.to_owned(),
    absolute,
    removed: false,
  })
}

/// The files with a `New` line, materialized; a failed one is skipped with a notice.
fn stage(
  env: &Env,
  root: &Path,
  temp: &TempDir,
  shaped: &Shaped,
) -> (Vec<EditedFile>, Vec<Notice>) {
  let top = toplevel(env, root).unwrap_or_else(|| root.to_path_buf());
  let mut files = Vec::new();
  let mut notices = Vec::new();
  for file in &shaped.files {
    if !file.lines.iter().any(|line| line.side == Side::New) {
      continue;
    }
    match materialize(env, &top, temp.path(), &file.path) {
      Ok(edited) => files.push(edited),
      Err(why) => notices.push(notice(
        NoticeKind::AstGrepFailed,
        format!("prepass: ast-grep skipped {}: {why}", file.path),
      )),
    }
  }
  (files, notices)
}

fn failure(failure: &ScanFailure) -> Notice {
  notice(
    NoticeKind::AstGrepFailed,
    format!(
      "prepass: ast-grep failed (exit {}): {}",
      failure.exit_code, failure.stderr_first
    ),
  )
}

/// Findings for the hits that sit on a changed `New` line, each once.
fn findings(shaped: &Shaped, hits: &[AstGrepHit]) -> Vec<Finding> {
  let mut seen = BTreeSet::new();
  hits
    .iter()
    .filter_map(|hit| {
      let path = hit.file.strip_prefix("./").unwrap_or(&hit.file);
      let number = u32::try_from(hit.line).ok()?;
      let file = shaped.files.iter().find(|file| file.path == path)?;
      let line = file
        .lines
        .iter()
        .find(|line| line.side == Side::New && line.line == number)?;
      seen.insert((path, number, &hit.rule_id)).then(|| {
        let (quote, redaction_spans) = redact_line(&line.text);
        Finding {
          id: finding_id(Source::AstGrep, path, number, &hit.rule_id),
          path: file.path.clone(),
          line: number,
          end_line: number,
          side: Side::New,
          severity: Severity::Medium,
          category: hit.rule_id.clone(),
          source: Source::AstGrep,
          claim: message(&hit.rule_id),
          quote,
          suggestion: None,
          redaction_spans,
          assessment: None,
        }
      })
    })
    .collect()
}

/// Structural findings on the changed lines of `shaped`, and a notice for every
/// way the scan degraded; a missing scanner is never a clean pass.
pub(super) fn scan(
  env: &Env,
  root: &Path,
  shaped: &Shaped,
  scan: Scan<'_>,
) -> (Vec<Finding>, Vec<Notice>) {
  let temp = match tempfile::tempdir() {
    Ok(temp) => temp,
    Err(err) => {
      let text = format!("prepass: ast-grep skipped: temp dir: {err}");
      return (Vec::new(), vec![notice(NoticeKind::AstGrepFailed, text)]);
    }
  };
  let (files, mut notices) = stage(env, root, &temp, shaped);
  if files.is_empty() {
    return (Vec::new(), notices);
  }
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env,
    config_root: temp.path(),
    project_root: temp.path(),
    cwd: Some(temp.path()),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let found = match scan(&files, RULES, &ctx) {
    AstGrepScan::Ok { hits, .. } => findings(shaped, &hits),
    AstGrepScan::Missing => {
      let text = "prepass: ast-grep not installed; structural rules skipped".to_owned();
      notices.push(notice(NoticeKind::MissingAstGrep, text));
      Vec::new()
    }
    AstGrepScan::Failed(why) => {
      notices.push(failure(&why));
      Vec::new()
    }
  };
  (found, notices)
}

#[cfg(test)]
#[path = "tests/astgrep_test.rs"]
mod tests;
