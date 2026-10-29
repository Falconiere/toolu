//! Branch memory, usage log, and shared v2 push-review write.

use crate::contract::{Finding, Omitted, Report, Source};
use crate::memory;
use crate::prepass::{NoticeKind, PrepassResult};
use crate::shape::{self, Shaped};
use crate::validate::Recap;
use serde_json::json;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;
use toolu_state::lock::{LockOptions, with_lock_strict};
use toolu_state::push_review::{Input as StateInput, write as write_state};

/// Context needed to persist one review and write the shared gate state.
pub(super) struct RunState<'a> {
  pub(super) env: &'a Env,
  pub(super) ctx: &'a Ctx,
  pub(super) roots: &'a Roots,
  pub(super) root: &'a Path,
  pub(super) shaped: &'a Shaped,
  pub(super) prepass: &'a PrepassResult,
  pub(super) scoped: bool,
  pub(super) memory_path: &'a Path,
}

/// Persist memory and usage, then refresh the push-review gate state.
pub(super) fn persist(
  state: &RunState<'_>,
  report: &mut Report,
  recap: &Recap,
) -> Result<(), String> {
  let RunState {
    env,
    ctx,
    roots,
    root,
    shaped,
    prepass,
    scoped,
    memory_path,
  } = *state;
  let complete = mark_coverage(report, scoped, shaped, prepass);
  let remembered: Vec<Finding> = report
    .findings
    .iter()
    .filter(|finding| finding.source != Source::System)
    .cloned()
    .collect();
  let saved = memory::record(
    memory_path,
    &memory::RecordInput {
      base: &shaped.base_ref,
      sha: &shaped.diff_sha,
      findings: &remembered,
      recap,
      verdict: report.verdict(),
      complete,
    },
  )
  .and_then(|()| log_usage(roots, root, &shaped.diff_sha, report));
  if saved.is_err() {
    state_blocker(report);
  }
  let count = report.blocking_count();
  let findings = serde_json::to_string(&report.findings).map_err(|err| err.to_string())?;
  let diff = (shaped.base_ref.as_str(), shaped.diff_sha.as_str());
  if let Err(err) = saved {
    write_gate((env, ctx), root, diff, count, &findings)
      .map_err(|gate| format!("{err}; failed to block push-review state: {gate}"))?;
    return Err(err);
  }
  write_gate((env, ctx), root, diff, count, &findings)
}

/// Mark `report` incomplete for every condition production gates on: a scoped
/// run, an uncommitted worktree, or a degraded pre-pass. Returns whether it is complete.
pub(crate) fn mark_coverage(
  report: &mut Report,
  scoped: bool,
  shaped: &Shaped,
  prepass: &PrepassResult,
) -> bool {
  if scoped || shaped.uncommitted || prepass_degraded(prepass) {
    report.coverage.unreviewed.push(Omitted {
      path: "(review)".to_owned(),
      reason: if scoped {
        "path-limited review"
      } else if shaped.uncommitted {
        "worktree has uncommitted changes"
      } else {
        "mechanical checks degraded"
      }
      .to_owned(),
    });
  }
  // Every incomplete condition above left an unreviewed entry.
  settle(report)
}

/// Whether `report` is complete; an incomplete one with no blocking finding gains a
/// system blocker, so its gate state can never pass.
pub(crate) fn settle(report: &mut Report) -> bool {
  let complete = report.is_complete();
  if !complete && report.blocking_count() == 0 {
    report.findings.push(super::output::system_blocker(
      "code-review:incomplete",
      "coverage",
      "The review did not cover a clean committed diff",
      "review incomplete",
    ));
  }
  complete
}

fn state_blocker(report: &mut Report) {
  if report.blocking_count() == 0 {
    report.findings.push(super::output::system_blocker(
      "code-review:state",
      "state",
      "The review state could not be saved",
      "state unavailable",
    ));
  }
}

/// Dismiss exactly one stored finding; only a complete unchanged review can refresh gate state.
pub(crate) fn dismiss(env: &Env, ctx: &Ctx, id: &str, reason: &str) -> Result<String, String> {
  let roots = Roots::new(env.clone(), ctx.host);
  let root = super::project_root(&roots)?;
  let path = super::branch_memory_path(env, &roots, &root)?;
  let state = memory::dismiss(&path, id, reason)?;
  if !state.complete || state.base_ref.is_empty() {
    return Ok(format!(
      "code-review: dismissed {id}; review coverage is incomplete, re-run review"
    ));
  }
  // No model call follows, so the hash and the dirty probe are all the shaped diff would add.
  let state_root = roots
    .project_state_root(None, Some(&root))
    .ok_or("state root unavailable")?;
  let dirty =
    shape::dirty_except_state(env, &root, &state_root).map_err(|err| format!("shape: {err:?}"))?;
  if dirty || diff_sha(env, &root, &state.base_ref).as_deref() != Some(state.diff_sha.as_str()) {
    return Ok(format!(
      "code-review: dismissed {id}; diff changed, re-run review"
    ));
  }
  let open: Vec<_> = state
    .memory
    .prior
    .iter()
    .filter(|finding| finding.status == memory::Status::Open)
    .collect();
  let count = open
    .iter()
    .filter(|finding| finding.severity.blocking())
    .count();
  let findings = serde_json::to_string(&open).map_err(|err| err.to_string())?;
  let diff = (state.base_ref.as_str(), state.diff_sha.as_str());
  write_gate((env, ctx), &root, diff, count, &findings)?;
  Ok(format!(
    "code-review: dismissed {id}; push-review state refreshed"
  ))
}

fn prepass_degraded(prepass: &PrepassResult) -> bool {
  prepass.overflow
    || prepass.notices.iter().any(|notice| {
      matches!(
        notice.kind,
        NoticeKind::MissingAstGrep | NoticeKind::AstGrepFailed | NoticeKind::SecretsCompile
      )
    })
}

fn log_usage(roots: &Roots, root: &Path, sha: &str, report: &Report) -> Result<(), String> {
  let dir = roots
    .project_state_root(None, Some(root))
    .ok_or_else(|| "state root unavailable".to_owned())?;
  std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
  let path = dir.join("code-review-usage.jsonl");
  let row = json!({"diff_sha": sha, "verdict": report.verdict(), "usage": report.usage});
  let line = format!("{row}\n");
  if line.len() > USAGE_LIMIT {
    return Err("usage record exceeds 1 MiB retention limit".to_owned());
  }
  with_lock_strict(
    &path,
    LockOptions {
      timeout: Duration::from_secs(5),
      stale: Duration::from_secs(3600),
    },
    || append_usage(&path, line.as_bytes()),
  )?
}

const USAGE_LIMIT: usize = 1_048_576;
const USAGE_ARCHIVES: usize = 4;

fn append_usage(path: &Path, line: &[u8]) -> Result<(), String> {
  let old_len = match std::fs::metadata(path) {
    Ok(meta) => meta.len(),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
    Err(err) => return Err(format!("usage metadata: {err}")),
  };
  let rotated = old_len
    .checked_add(u64::try_from(line.len()).map_err(|err| err.to_string())?)
    .is_none_or(|size| size > USAGE_LIMIT as u64);
  if rotated && old_len > 0 {
    rotate_usage(path)?;
  }
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(path)
    .map_err(|err| format!("usage append open: {err}"))?;
  if let Err(err) = file.write_all(line) {
    let prior = if rotated { 0 } else { old_len };
    return match file.set_len(prior) {
      Ok(()) => Err(format!("usage append: {err}")),
      Err(rollback) => Err(format!("usage append: {err}; rollback: {rollback}")),
    };
  }
  prune_usage(path)
}

fn usage_archives(path: &Path) -> Result<Vec<(u64, PathBuf, u64)>, String> {
  let dir = path.with_file_name("code-review-usage");
  let entries = match std::fs::read_dir(&dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(format!("usage archives: {err}")),
  };
  let mut archives = Vec::new();
  for entry in entries {
    let entry = entry.map_err(|err| format!("usage archives: {err}"))?;
    let name = entry.file_name();
    let Some(sequence) = name
      .to_str()
      .and_then(|name| name.strip_suffix(".jsonl"))
      .filter(|digits| digits.len() == 20 && digits.bytes().all(|byte| byte.is_ascii_digit()))
      .and_then(|digits| digits.parse::<u64>().ok())
    else {
      continue;
    };
    let meta = entry
      .metadata()
      .map_err(|err| format!("usage archives: {err}"))?;
    if !meta.is_file() {
      return Err(format!(
        "usage archive {} is not a file",
        entry.path().display()
      ));
    }
    archives.push((sequence, entry.path(), meta.len()));
  }
  archives.sort_by_key(|(sequence, _, _)| *sequence);
  Ok(archives)
}

fn rotate_usage(path: &Path) -> Result<(), String> {
  let dir = path.with_file_name("code-review-usage");
  std::fs::create_dir_all(&dir).map_err(|err| format!("usage rotation: {err}"))?;
  let next = usage_archives(path)?
    .last()
    .map_or(Some(1), |(sequence, _, _)| sequence.checked_add(1))
    .ok_or("usage archive sequence exhausted")?;
  let target = dir.join(format!("{next:020}.jsonl"));
  std::fs::rename(path, target).map_err(|err| format!("usage rotation: {err}"))
}

fn prune_usage(path: &Path) -> Result<(), String> {
  let archives = usage_archives(path)?;
  prune_archives(&archives)
}

fn prune_archives(archives: &[(u64, PathBuf, u64)]) -> Result<(), String> {
  let excess = archives.len().saturating_sub(USAGE_ARCHIVES);
  for (index, (_, archive, len)) in archives.iter().enumerate() {
    if index < excess || *len > USAGE_LIMIT as u64 {
      std::fs::remove_file(archive).map_err(|err| format!("usage cleanup: {err}"))?;
    }
  }
  Ok(())
}

/// `(base, sha)` is the reviewed diff; the writer refuses a different current hash.
fn write_gate(
  (env, ctx): (&Env, &Ctx),
  root: &Path,
  (base, sha): (&str, &str),
  count: usize,
  findings: &str,
) -> Result<(), String> {
  let input = StateInput {
    findings_count: u64::try_from(count).unwrap_or(u64::MAX),
    reviewers: Ordered::parse("[\"code-review\",\"code-review:native\"]")?,
    findings: Ordered::parse(findings)?,
    repo: root
      .to_str()
      .ok_or_else(|| "non-UTF8 repository path".to_owned())?,
    branch: None,
    reviewed_files: None,
    expected_diff_sha: Some(sha),
  };
  let env = env.clone().with("PUSH_REVIEW_BASE", base);
  write_state(&input, ctx, &env).map(|_| ())
}

#[cfg(test)]
#[path = "tests/state_test.rs"]
mod tests;
