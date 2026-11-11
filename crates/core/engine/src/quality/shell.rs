//! Source paths changed by one shell command, selected before the shared quality batch.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::GateFile;

use super::QualityBinding;
use super::run::gate_path;

mod quiet;
use quiet::{has_extension, quiet};

const MAX_FILES: usize = 20;

/// Candidate paths and diagnostics from one non-quiet shell event.
pub(crate) struct Selection {
  pub(crate) records: Vec<EditRecord>,
  pub(crate) advisory: Option<String>,
  pub(crate) warnings: Vec<String>,
  pub(crate) gate: PathBuf,
}

fn ctime(path: &Path) -> Result<Option<(i64, i64)>, std::io::Error> {
  match fs::symlink_metadata(path) {
    Ok(meta) => Ok(Some((meta.ctime(), meta.ctime_nsec()))),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(error),
  }
}

fn baseline(marker: &Path, warnings: &mut Vec<String>) -> Option<(i64, i64)> {
  match ctime(marker) {
    Ok(before) => before,
    Err(error) => {
      warnings.push(format!(
        "quality: cannot read {}: {error}",
        marker.display()
      ));
      None
    }
  }
}

fn status(ctx: &RuleContext<'_>) -> Result<Vec<String>, String> {
  let mut spec = Spec::new([
    "git",
    "--no-optional-locks",
    "status",
    "--porcelain=v1",
    "-z",
    "--no-renames",
    "--untracked-files=all",
  ]);
  spec.cwd = Some(ctx.project_root.to_path_buf());
  spec.env = Some(ctx.env.clone());
  spec.timeout = Duration::from_secs(5);
  let output = run(&spec).map_err(|error| format!("quality: git status failed: {error:?}"))?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Err(format!(
      "quality: git status failed (exit {}, timeout {}, truncated {})",
      output.exit_code, output.timed_out, output.truncated
    ));
  }
  let bytes = output.stdout_bytes;
  if !bytes.is_empty() && bytes.last() != Some(&0) {
    return Err("quality: malformed git status output".to_owned());
  }
  let mut paths = Vec::new();
  for entry in bytes
    .split(|byte| *byte == 0)
    .filter(|entry| !entry.is_empty())
  {
    if entry.len() < 4 || entry[2] != b' ' {
      return Err("quality: malformed git status output".to_owned());
    }
    let path = std::str::from_utf8(&entry[3..])
      .map_err(|_| "quality: non-UTF-8 git status path".to_owned())?;
    paths.push(path.to_owned());
  }
  Ok(paths)
}

fn record(path: String, operation: EditOperation) -> EditRecord {
  EditRecord {
    path,
    operation,
    from: None,
    moved_to: None,
  }
}

fn missing_gate_paths(
  gate: &Path,
  root: &Path,
  active: &[usize],
  bindings: &[QualityBinding<'_>],
  warnings: &mut Vec<String>,
) -> Vec<EditRecord> {
  let slots = match read_gate_file(gate) {
    GateRead::Ok(GateFile::Failing {
      file,
      source,
      entries,
      ..
    }) => entries.map_or_else(
      || vec![(file, source)],
      |entries| {
        entries
          .into_iter()
          .map(|(file, entry)| (file, entry.source))
          .collect()
      },
    ),
    GateRead::Malformed(reason) | GateRead::Unrecognized { reason, .. } => {
      warnings.push(format!("quality: cannot read gate entries: {reason}"));
      Vec::new()
    }
    GateRead::Missing | GateRead::Ok(GateFile::Passing { .. }) => Vec::new(),
  };
  slots
    .into_iter()
    .filter(|(file, source)| {
      has_extension(file, bindings, active)
        && active.iter().any(|index| {
          bindings
            .get(*index)
            .is_some_and(|binding| binding.rule.source() == source)
        })
        && !root.join(file).exists()
    })
    .map(|(file, _)| record(file, EditOperation::Delete))
    .collect()
}

fn live_paths(
  paths: Vec<String>,
  root: &Path,
  before: (i64, i64),
  active: &[usize],
  bindings: &[QualityBinding<'_>],
  warnings: &mut Vec<String>,
) -> Vec<String> {
  let mut live = Vec::new();
  for path in paths {
    if !has_extension(&path, bindings, active) {
      continue;
    }
    let absolute = root.join(&path);
    if !fs::metadata(&absolute).is_ok_and(|meta| meta.is_file()) {
      continue;
    }
    match ctime(&absolute) {
      Ok(Some(changed)) if changed >= before => live.push(path),
      Ok(_) => {}
      Err(error) => warnings.push(format!("quality: cannot inspect {path}: {error}")),
    }
  }
  live.sort();
  live.dedup();
  live
}

fn selected_records(
  live: Vec<String>,
  gate: &Path,
  root: &Path,
  active: &[usize],
  bindings: &[QualityBinding<'_>],
  warnings: &mut Vec<String>,
) -> (Vec<EditRecord>, Option<String>) {
  let advisory = (live.len() > MAX_FILES).then(|| {
    format!(
      "quality: {} changed source files were not checked: {}",
      live.len() - MAX_FILES,
      live[MAX_FILES..].join(", ")
    )
  });
  let mut records: Vec<EditRecord> = live
    .into_iter()
    .take(MAX_FILES)
    .map(|path| record(path, EditOperation::Write))
    .collect();
  let mut seen: HashSet<String> = records.iter().map(|record| record.path.clone()).collect();
  for removed in missing_gate_paths(gate, root, active, bindings, warnings) {
    if seen.insert(removed.path.clone()) {
      records.push(removed);
    }
  }
  (records, advisory)
}

/// Candidate records for a non-quiet shell post event; `None` means no Git or marker work.
pub(crate) fn collect(
  ctx: &RuleContext<'_>,
  active: &[usize],
  bindings: &[QualityBinding<'_>],
) -> Option<Selection> {
  if quiet(ctx, bindings) {
    return None;
  }
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let gate = gate_path(ctx.project_root, &roots);
  let marker = gate.with_file_name("quality-shell-mark");
  let mut warnings = Vec::new();
  let before = baseline(&marker, &mut warnings);
  let paths = match status(ctx) {
    Ok(paths) => Some(paths),
    Err(warning) => {
      warnings.push(warning);
      None
    }
  };
  let (Some(before), Some(paths)) = (before, paths) else {
    return Some(Selection {
      records: Vec::new(),
      advisory: None,
      warnings,
      gate,
    });
  };
  let live = live_paths(
    paths,
    ctx.project_root,
    before,
    active,
    bindings,
    &mut warnings,
  );
  let (records, advisory) = selected_records(
    live,
    &gate,
    ctx.project_root,
    active,
    bindings,
    &mut warnings,
  );
  Some(Selection {
    records,
    advisory,
    warnings,
    gate,
  })
}

/// Advance the shell baseline after decisions and after each edit quality settlement.
pub(crate) fn touch_marker(gate: &Path, warnings: &mut Vec<String>) {
  let marker = gate.with_file_name("quality-shell-mark");
  if let Some(parent) = marker.parent()
    && let Err(error) = fs::create_dir_all(parent)
  {
    warnings.push(format!(
      "quality: cannot create {}: {error}",
      parent.display()
    ));
    return;
  }
  if let Err(error) = fs::write(&marker, b"1") {
    warnings.push(format!(
      "quality: cannot touch {}: {error}",
      marker.display()
    ));
  }
}

#[cfg(test)]
#[path = "tests/shell_test.rs"]
mod tests;
