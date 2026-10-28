//! The review-memory records carried between rounds in the per-branch code-review
//! state file, chained by `diff_sha` (epic #660 review memory). #666 and #668 read
//! them and #669 persists them; the push-review gate file never holds them.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use toolu_runtime::atomic::replace_atomic;
use toolu_runtime::host::roots::Roots;
use toolu_state::git::branch_slug;
use toolu_state::lock::{LockOptions, with_lock_strict};

use crate::contract::Finding;
use crate::validate::{Recap, fingerprint};

use crate::contract::{Severity, Verdict};

/// Where a prior finding stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
  /// Still open: not fixed, or not re-examined this round.
  Open,
  /// Fixed in a later diff.
  Resolved,
  /// Settled by an explicit dismissal.
  Dismissed,
}

/// A finding from an earlier round. `path`, `category` and `claim` let a later
/// round show the model what was raised, so it can echo `prior_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorFinding {
  /// Line-independent: `path + category + normalized text` (normalized by #668).
  pub fingerprint: String,
  /// The finding's id.
  pub id: String,
  /// Repository-relative path.
  pub path: String,
  /// Its kind, for example `correctness`.
  pub category: String,
  /// The defect, in one sentence.
  pub claim: String,
  /// Its severity.
  pub severity: Severity,
  /// Where it stands.
  pub status: Status,
}

/// An agent's dismissal of a finding; a blocker matches by exact id only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dismissal {
  /// The dismissed finding's id.
  pub id: String,
  /// Why it does not apply.
  pub reason: String,
  /// Who dismissed it.
  pub by: String,
}

/// What a review round remembers from earlier ones: prior findings and the
/// agent's dismissals. Only open findings and dismissals are ever sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
  /// Findings from earlier rounds.
  pub prior: Vec<PriorFinding>,
  /// Settled dismissals, with their reasons.
  pub dismissals: Vec<Dismissal>,
}

/// Blocking and advisory finding counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
  /// `blocker`, `high` and `medium`.
  pub blocking: usize,
  /// `low` and `nit`.
  pub advisory: usize,
}

/// One past review pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pass {
  /// The reviewed diff's hash, as the push-review gate computes it.
  pub diff_sha: String,
  /// The pass's verdict.
  pub verdict: Verdict,
  /// Its finding counts.
  pub counts: Counts,
}

/// Code-review's separate branch file; the push-review writer cannot erase it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BranchState {
  /// Base ref used for the latest review.
  pub base_ref: String,
  /// The most recent committed diff hash.
  pub diff_sha: String,
  /// Whether the latest run covered the whole clean committed diff.
  pub complete: bool,
  /// Findings and explicit dismissals across diff hashes.
  pub memory: Memory,
  /// The last five passes, oldest first.
  pub passes: Vec<Pass>,
}

/// One completed review to persist in branch memory.
pub struct RecordInput<'a> {
  /// Base ref of the committed review diff.
  pub base: &'a str,
  /// Exact committed diff hash.
  pub sha: &'a str,
  /// Findings after validation.
  pub findings: &'a [Finding],
  /// Cross-round status changes.
  pub recap: &'a Recap,
  /// Derived verdict.
  pub verdict: Verdict,
  /// Whether the whole clean committed diff was reviewed.
  pub complete: bool,
}

/// The per-branch memory file in the host's project state root.
///
/// # Errors
/// The host cannot resolve a project state root.
pub fn path(roots: &Roots, root: &Path, branch: &str) -> Result<PathBuf, String> {
  let dir = roots
    .project_state_dir("code-review", None, Some(root))
    .map_err(|err| err.0)?
    .ok_or_else(|| "code-review state root unavailable".to_owned())?;
  Ok(dir.join(format!("{}.json", branch_slug(branch))))
}

/// Read the memory file; malformed history fails closed.
///
/// # Errors
/// A present file cannot be read or parsed.
pub fn load(path: &Path) -> Result<BranchState, String> {
  match std::fs::read_to_string(path) {
    Ok(text) => serde_json::from_str(&text).map_err(|err| format!("memory: {err}")),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BranchState::default()),
    Err(err) => Err(format!("memory: {err}")),
  }
}

/// Persist one validated pass under the shared lock protocol.
///
/// # Errors
/// Reading, locking, serializing, or writing the branch file fails.
pub fn record(path: &Path, input: &RecordInput<'_>) -> Result<(), String> {
  locked(path, || {
    let mut state = load(path)?;
    update_findings(&mut state.memory, input.findings, input.recap);
    let blocking = input
      .findings
      .iter()
      .filter(|finding| finding.severity.blocking())
      .count();
    state.passes.push(Pass {
      diff_sha: input.sha.to_owned(),
      verdict: input.verdict,
      counts: Counts {
        blocking,
        advisory: input.findings.len() - blocking,
      },
    });
    if state.passes.len() > 5 {
      state.passes.drain(..state.passes.len() - 5);
    }
    input.sha.clone_into(&mut state.diff_sha);
    input.base.clone_into(&mut state.base_ref);
    state.complete = input.complete;
    write(path, &state)
  })
}

fn update_findings(memory: &mut Memory, findings: &[Finding], recap: &Recap) {
  for prior in &mut memory.prior {
    if recap.resolved.contains(&prior.id) {
      prior.status = Status::Resolved;
    }
  }
  for finding in findings {
    let prior = PriorFinding {
      fingerprint: fingerprint(&finding.path, &finding.category, &finding.claim),
      id: finding.id.clone(),
      path: finding.path.clone(),
      category: finding.category.clone(),
      claim: finding.claim.clone(),
      severity: finding.severity,
      status: Status::Open,
    };
    if let Some(old) = memory.prior.iter_mut().find(|old| old.id == finding.id) {
      if old.status != Status::Dismissed {
        *old = prior;
      }
    } else {
      memory.prior.push(prior);
    }
  }
}

/// Settle one prior finding by exact ID, without a model call.
///
/// # Errors
/// The ID or reason is invalid, or branch state cannot be updated.
pub fn dismiss(path: &Path, id: &str, reason: &str) -> Result<BranchState, String> {
  if id.starts_with("prepass:") {
    return Err("deterministic findings cannot be dismissed".to_owned());
  }
  if reason.trim().is_empty() {
    return Err("dismissal reason is empty".to_owned());
  }
  locked(path, || {
    let mut state = load(path)?;
    let prior = state
      .memory
      .prior
      .iter_mut()
      .find(|prior| prior.id == id)
      .ok_or_else(|| format!("finding {id} is not in branch memory"))?;
    prior.status = Status::Dismissed;
    state.memory.dismissals.retain(|item| item.id != id);
    state.memory.dismissals.push(Dismissal {
      id: id.to_owned(),
      reason: reason.to_owned(),
      by: "agent".to_owned(),
    });
    write(path, &state)?;
    Ok(state)
  })
}

fn write(path: &Path, state: &BranchState) -> Result<(), String> {
  let body = serde_json::to_string(state).map_err(|err| err.to_string())?;
  replace_atomic(path, &(body + "\n")).map_err(|err| format!("memory: {err}"))
}

fn locked<T>(path: &Path, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
  }
  with_lock_strict(path, LockOptions::default(), f)?
}

#[cfg(test)]
#[path = "tests/memory_test.rs"]
mod tests;
