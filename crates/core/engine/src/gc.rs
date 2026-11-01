//! Reclaim what finished epics, gates and sandboxes leave behind: merged
//! worktrees and branches, the `refs/epic-wip/*` snapshots of merged issues,
//! old job logs, and idle VMs. [`sweep`] reports by default and acts with
//! `apply`; `toolu gc` and the epic engine (after `epic-closed`) both call it.
//! Every step tolerates "already gone", so a repeated sweep is harmless.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, run};

use crate::resources::store::{leases, read_resource_state};

pub mod branches;
pub mod engine_state;
pub mod refs;
pub mod repos;
pub mod vms;
pub mod worktrees;

/// How long one git or herdr query may take.
const QUERY: Duration = Duration::from_secs(60);

/// What one sweep covers.
#[derive(Debug, Clone)]
pub struct Request {
  /// Repository checkouts to sweep, besides those the issue records name.
  pub repos: Vec<PathBuf>,
  /// Epic state directories whose issue records name checkouts, keys and stages.
  pub state_dirs: Vec<PathBuf>,
  /// The engine's resource root: leases and job logs.
  pub resource_root: PathBuf,
  /// The `toolu` that runs `sandbox prune` and `sandbox stop`; `None` skips the sandbox.
  pub toolu: Option<PathBuf>,
  /// Remove and stop; otherwise only report what would go.
  pub apply: bool,
  /// Delete every merged local branch, not only the issue records' branches.
  pub all_branches: bool,
  /// The environment child processes run with; `colima`, `docker` and `herdr` come from its `PATH`.
  pub env: Env,
}

/// Sweep everything `request` names and describe each decision.
pub fn sweep(request: &Request) -> Value {
  let records = repos::records(&request.state_dirs);
  let roots = repos::discover(&request.env, &request.repos, &records);
  let held = held_worktrees(&request.resource_root, &records);
  let repos: Vec<Value> = roots
    .iter()
    .map(|root| {
      let mut doc = worktrees::sweep(request, root, &held);
      if let Value::Object(map) = &mut doc
        && !map.contains_key("skipped")
      {
        map.insert(
          "branches".to_owned(),
          branches::sweep(request, root, &records),
        );
        map.insert("wip_refs".to_owned(), refs::sweep(request, root, &records));
      }
      doc
    })
    .collect();
  let idle = vms::idle(&request.resource_root);
  json!({
    "apply": request.apply,
    "repos": repos,
    "sandbox": vms::sandbox(request, &idle),
    "colima": vms::colima(request, &idle),
    "docker": vms::docker(request),
    "engine": engine_state::sweep(request, &records),
  })
}

/// The worktrees a lease under `root` or a busy issue record still names.
fn held_worktrees(root: &Path, records: &[repos::IssueRecord]) -> BTreeSet<String> {
  let mut held: BTreeSet<String> = records
    .iter()
    .filter(|record| !record.terminal())
    .filter_map(|record| record.worktree.as_ref())
    .map(|path| path.display().to_string())
    .collect();
  if let Ok(state) = read_resource_state(root) {
    for lease in leases(&state) {
      if let Some(Ordered::String(path)) = lease.get("worktree") {
        held.insert(path.clone());
      }
    }
  }
  held
}

/// One line for a journal note: what went (or would go) and the VM outcomes.
pub fn summary(report: &Value) -> String {
  let repos = report
    .get("repos")
    .and_then(Value::as_array)
    .map(Vec::as_slice)
    .unwrap_or_default();
  let count = |section: &str, field: &str| -> usize {
    repos
      .iter()
      .filter_map(|repo| repo.get(section).and_then(|part| part.get(field)))
      .filter_map(Value::as_array)
      .map(Vec::len)
      .sum()
  };
  let trees: usize = repos
    .iter()
    .filter_map(|repo| repo.get("worktrees").and_then(Value::as_array))
    .flatten()
    .filter(|tree| {
      matches!(
        tree.get("action").and_then(Value::as_str),
        Some("removed" | "would-remove")
      )
    })
    .count();
  let text = |path: &[&str]| -> String {
    let mut node = report;
    for key in path {
      node = node.get(key).unwrap_or(&Value::Null);
    }
    node.as_str().unwrap_or("skipped").to_owned()
  };
  format!(
    "{}worktrees={trees} branches={} wip_refs={} job_logs={} sandbox={} colima={}",
    if report.get("apply") == Some(&Value::Bool(true)) {
      ""
    } else {
      "would: "
    },
    count("branches", "removed"),
    count("wip_refs", "removed"),
    report
      .pointer("/engine/job_logs_removed")
      .and_then(Value::as_u64)
      .unwrap_or(0),
    text(&["sandbox", "action"]),
    text(&["colima", "toolu-gate"]),
  )
}

/// `argv`'s stdout when it exits 0 in time; otherwise why it did not, in words.
pub(crate) fn capture(
  env: &Env,
  cwd: Option<&Path>,
  argv: &[&str],
  timeout: Duration,
) -> Result<String, String> {
  let mut spec = Spec::new(argv.iter().map(|word| (*word).to_owned()));
  spec.cwd = cwd.map(Path::to_path_buf);
  spec.env = Some(env.clone());
  spec.timeout = timeout;
  let output = run(&spec).map_err(|err| format!("`{}` cannot run: {err:?}", argv.join(" ")))?;
  if output.exit_code == 0 && !output.timed_out {
    return Ok(output.stdout);
  }
  let why = output.stderr.trim();
  Err(format!(
    "`{}` failed: {}",
    argv.join(" "),
    if output.timed_out {
      "it timed out"
    } else if why.is_empty() {
      "no message"
    } else {
      why
    }
  ))
}

/// `git -C root args`.
pub(crate) fn git(env: &Env, root: &Path, args: &[&str]) -> Result<String, String> {
  let dir = root.to_string_lossy();
  let mut words = vec!["git", "-C", dir.as_ref()];
  words.extend_from_slice(args);
  capture(env, None, &words, QUERY)
}

/// Whether `git -C root args` succeeds.
pub(crate) fn git_ok(env: &Env, root: &Path, args: &[&str]) -> bool {
  git(env, root, args).is_ok()
}

#[cfg(test)]
#[path = "tests/gc_support_test.rs"]
mod support;

#[cfg(test)]
#[path = "tests/gc_test.rs"]
mod tests;
