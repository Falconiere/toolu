//! The transient-state sweeper for `<root>/<host dir>/tmp` (`state-sweeper.ts`),
//! run once per session. It reclaims only state that is provably spent:
//!
//! - `push-review/`, `plan-ledger/`, `docs-sync/`: per-branch `*.json`, dropped
//!   when the branch is merged into base, gone, or older than the TTL; the
//!   current branch is never touched;
//! - `quality-gate-status.json`: dropped when passing, or failing only about
//!   files that no longer exist; an unrecognized document is kept;
//! - `telemetry/*.jsonl`: trimmed to the retention window (`sweep_telemetry`).
//!
//! Best effort: failures become `toolu-sweep: …` warnings, never errors.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use toolu_runtime::config::load::LoadedConfig;
use toolu_runtime::config::read::enabled;

use crate::ctx::StateCtx;
use crate::gate_file::{GateRead, read_gate_file};
use crate::gate_schema::{GLOBAL_GATE_KEY, GateFile};
use crate::git::{base_branch, branch_slug, branch_slugs, current_branch, has_git};
use crate::lock::{LockOptions, with_lock};
pub use crate::sweep_telemetry::kept_telemetry_lines;
use crate::sweep_telemetry::sweep_telemetry;

/// Hours a live, unmerged branch's state is kept (`gates.stateTtlHours`).
pub const SWEEP_DEFAULT_TTL_HOURS: u64 = 24;
/// Days telemetry is kept (`gates.telemetryRetentionDays`).
pub const SWEEP_DEFAULT_RETENTION_DAYS: u64 = 7;
/// The per-branch state directories.
pub const SWEEP_BRANCH_DIRS: [&str; 3] = ["push-review", "plan-ledger", "docs-sync"];

const HOUR: Duration = Duration::from_hours(1);

/// `gates.<key>` when it is a number above 0, floored; else `fallback`.
fn positive_gate(config: &LoadedConfig, key: &str, fallback: u64) -> u64 {
  let value = config.data.get("gates").and_then(|gates| gates.get(key));
  let number = value
    .and_then(serde_json::Value::as_f64)
    .filter(|number| *number > 0.0);
  number.map_or(fallback, |number| {
    number.floor().to_string().parse().unwrap_or(u64::MAX)
  })
}

/// Non-dot regular files in `dir` ending in `suffix` (`*.suffix` plus `[ -f ]`).
pub(crate) fn glob_files(dir: &Path, suffix: &str) -> Vec<PathBuf> {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return Vec::new();
  };
  let mut files: Vec<PathBuf> = entries
    .flatten()
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| name.ends_with(suffix) && !name.starts_with('.'))
    .map(|name| dir.join(name))
    .filter(|file| file.is_file())
    .collect();
  files.sort();
  files
}

/// The branch slug a state file belongs to: `feat_x.waiver.json` and
/// `feat_x.pending-waiver.json` are `feat_x`.
pub fn slug_of_state_file(file: &Path) -> String {
  let name = file
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default();
  let name = name.strip_suffix(".json").unwrap_or(&name);
  let name = name.strip_suffix(".pending-waiver").unwrap_or(name);
  name.strip_suffix(".waiver").unwrap_or(name).to_owned()
}

/// Removes `file`, or warns that it could not.
pub(crate) fn remove(file: &Path, warnings: &mut Vec<String>) {
  if std::fs::remove_file(file).is_err() {
    warnings.push(format!("toolu-sweep: could not remove {}", file.display()));
  }
}

/// The branches a sweep judges by: the current one, the live ones, the merged ones.
struct Branches {
  current: String,
  live: BTreeSet<String>,
  merged: BTreeSet<String>,
}

/// Whether `file` (of branch `slug`) is spent; `Err` when it cannot be judged.
fn reclaimable(
  file: &Path,
  slug: &str,
  branches: &Branches,
  ttl: Duration,
  now: SystemTime,
) -> std::io::Result<bool> {
  if branches.merged.contains(slug) || !branches.live.contains(slug) {
    return Ok(true);
  }
  // A live, unmerged branch: age is the only remaining reason.
  let modified = std::fs::metadata(file)?.modified()?;
  Ok(now.duration_since(modified).is_ok_and(|age| age > ttl))
}

fn sweep_branch_state(ctx: &mut StateCtx, root: &Path, state_root: &Path, ttl_hours: u64) {
  let env = ctx.roots.env().clone();
  let branches = Branches {
    current: branch_slug(&current_branch(&env, root)),
    live: branch_slugs(&env, root, None),
    // `--merged` sees ancestry only; squash-merged branches age out through the TTL.
    merged: branch_slugs(&env, root, Some(&base_branch(&env, Some(root), root))),
  };
  let ttl = HOUR.saturating_mul(u32::try_from(ttl_hours).unwrap_or(u32::MAX));
  let now = ctx.now();
  for dir in SWEEP_BRANCH_DIRS {
    for file in glob_files(&state_root.join(dir), ".json") {
      let slug = slug_of_state_file(&file);
      if slug == branches.current {
        continue;
      }
      match reclaimable(&file, &slug, &branches, ttl, now) {
        Ok(true) => remove(&file, &mut ctx.warnings),
        Ok(false) => {}
        Err(err) => ctx.warnings.push(format!(
          "toolu-sweep: could not judge {}: {err}",
          file.display()
        )),
      }
    }
  }
}

/// Whether a failing record names a live file (`__global__` is always live).
fn has_live_violation(keys: &[String]) -> bool {
  keys
    .iter()
    .any(|key| key == GLOBAL_GATE_KEY || (!key.is_empty() && Path::new(key).exists()))
}

fn sweep_gate_file(gate: &Path, warnings: &mut Vec<String>) {
  if !gate.exists() {
    return;
  }
  let mut lock_warnings = Vec::new();
  let mut removals = Vec::new();
  with_lock(gate, LockOptions::default(), &mut lock_warnings, || {
    let GateRead::Ok(doc) = read_gate_file(gate) else {
      return;
    };
    let spent = match &doc {
      GateFile::Passing { .. } => true,
      GateFile::Failing {
        entries: Some(entries),
        ..
      } => !has_live_violation(
        &entries
          .iter()
          .map(|(key, _)| key.clone())
          .collect::<Vec<_>>(),
      ),
      // Age alone never clears a failure: that would silently reopen a real violation.
      GateFile::Failing {
        file,
        entries: None,
        ..
      } => !has_live_violation(std::slice::from_ref(file)),
    };
    if spent {
      remove(gate, &mut removals);
    }
  });
  warnings.extend(lock_warnings);
  warnings.extend(removals);
}

/// `toolu_sweep_state ROOT`: reclaims spent transient state of `root`, else
/// of the project root. Never fails; problems become warnings.
pub fn sweep_state(ctx: &mut StateCtx, root: Option<&Path>) {
  let root = match root.filter(|root| !root.as_os_str().is_empty()) {
    Some(root) => root.to_path_buf(),
    None => match ctx.roots.project_root(None) {
      Some(root) => root,
      None => return,
    },
  };
  let config = ctx.config(&root);
  let (ttl, retention) = (
    positive_gate(config, "stateTtlHours", SWEEP_DEFAULT_TTL_HOURS),
    positive_gate(
      config,
      "telemetryRetentionDays",
      SWEEP_DEFAULT_RETENTION_DAYS,
    ),
  );
  if !enabled(config, "gates", "sweep") || !has_git(ctx.roots.env()) {
    return;
  }
  let Some(state_root) = ctx
    .roots
    .project_state_root(None, Some(&root))
    .filter(|dir| dir.is_dir())
  else {
    return;
  };
  sweep_branch_state(ctx, &root, &state_root, ttl);
  sweep_gate_file(
    &state_root.join("quality-gate-status.json"),
    &mut ctx.warnings,
  );
  sweep_telemetry(ctx, &state_root.join("telemetry"), retention);
}

#[cfg(test)]
#[path = "tests/sweeper_test.rs"]
mod tests;
