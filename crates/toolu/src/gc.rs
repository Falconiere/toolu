//! `toolu gc`: reclaim what finished epics, gates and sandboxes leave behind.
//! A report by default; `--apply` removes merged worktrees and branches, the
//! `refs/epic-wip/*` snapshots of finished issues and old job logs, prunes the
//! sandbox's orphan clones, and stops the sandbox and the Colima gate profile
//! when no worker holds a lease. The epic engine runs the same sweep after it
//! closes an epic (`toolu_engine::gc`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::Value;
use toolu_engine::gc::repos::state_dirs_under;
use toolu_engine::gc::{Request, summary, sweep};
use toolu_engine::resources::store::resource_home;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_exe;
use toolu_runtime::process::commands::git_toplevel;

/// Reclaim leftovers of finished epics, gates and sandboxes.
pub fn command() -> Command {
  Command::new("gc")
    .about("Reclaim what finished epics, gates and sandboxes leave behind; a report unless --apply")
    .arg(
      Arg::new("apply")
        .long("apply")
        .action(ArgAction::SetTrue)
        .help("Remove merged worktrees, branches, snapshots and old job logs; stop idle VMs"),
    )
    .arg(
      Arg::new("repo")
        .long("repo")
        .value_name("PATH")
        .action(ArgAction::Append)
        .help("A checkout to sweep, besides this one and those the epic records name"),
    )
}

/// Sweep and print the report: text, or the document under `--json`.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let (roots, cwd) = crate::overlaid_roots(ctx);
  let env = roots.env().clone();
  let resource_root = resource_home(&env);
  let mut repos: Vec<PathBuf> = matches
    .get_many::<String>("repo")
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect();
  repos.extend(git_toplevel(&env, &cwd));
  let request = Request {
    repos,
    state_dirs: state_dirs(&env, &resource_root),
    resource_root,
    toolu: current_exe(),
    apply: matches.get_flag("apply"),
    all_branches: true,
    env,
  };
  let report = sweep(&request);
  if ctx.json {
    Outcome::data(report.to_string())
  } else {
    Outcome::data(lines(&report).join("\n"))
  }
}

/// The epic state directories: the registered ones, their siblings, and those
/// under `EPIC_STATE_HOME` (else `~/.claude/epics`).
pub(crate) fn state_dirs(env: &Env, resource_root: &Path) -> Vec<PathBuf> {
  let mut parents = BTreeSet::new();
  let mut dirs = BTreeSet::new();
  let registry = std::fs::read_to_string(resource_root.join("registry.json"))
    .ok()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
  for row in registry
    .as_ref()
    .and_then(|doc| doc.get("epics"))
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
  {
    if let Some(dir) = row.get("state_dir").and_then(Value::as_str) {
      let dir = PathBuf::from(dir);
      if let Some(parent) = dir.parent() {
        parents.insert(parent.to_path_buf());
      }
      dirs.insert(dir);
    }
  }
  parents.insert(
    env
      .get("EPIC_STATE_HOME")
      .map_or_else(|| env.home().join(".claude").join("epics"), PathBuf::from),
  );
  dirs.extend(state_dirs_under(&parents.into_iter().collect::<Vec<_>>()));
  dirs.into_iter().collect()
}

fn strings(value: Option<&Value>) -> Vec<String> {
  value
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::to_owned)
    .collect()
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
  value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The report for a person: the summary, then each repository, VM and record.
pub(crate) fn lines(report: &Value) -> Vec<String> {
  let mut out = vec![format!("gc: {}", summary(report))];
  let repos = report.get("repos").and_then(Value::as_array);
  for repo in repos.into_iter().flatten() {
    out.push(format!("{}:", text(repo, "root")));
    match repo.get("skipped").and_then(Value::as_str) {
      Some(why) => out.push(format!("  skipped: {why}")),
      None => out.extend(repo_lines(repo)),
    }
  }
  out.extend(sandbox_lines(report));
  out.extend(engine_lines(report));
  out
}

/// Worktree decisions (foreign ones aside), branches and snapshot counts.
fn repo_lines(repo: &Value) -> Vec<String> {
  let mut out = Vec::new();
  let trees = repo.get("worktrees").and_then(Value::as_array);
  for tree in trees.into_iter().flatten() {
    if text(tree, "reason") != "not made by toolu" {
      let (action, path, why) = (
        text(tree, "action"),
        text(tree, "path"),
        text(tree, "reason"),
      );
      out.push(format!("  worktree {action} {path}: {why}"));
    }
  }
  let branches = strings(repo.pointer("/branches/removed"));
  if !branches.is_empty() {
    out.push(format!("  branches: {}", branches.join(" ")));
  }
  let kept = repo.pointer("/branches/kept").and_then(Value::as_array);
  for branch in kept.into_iter().flatten() {
    let (name, why) = (text(branch, "name"), text(branch, "reason"));
    out.push(format!("  branch kept {name}: {why}"));
  }
  let refs = strings(repo.pointer("/wip_refs/removed")).len();
  let kept = repo
    .pointer("/wip_refs/kept")
    .and_then(Value::as_array)
    .map_or(0, Vec::len);
  if refs + kept > 0 {
    out.push(format!("  wip refs: {refs} to remove, {kept} kept"));
  }
  out
}

fn len(value: Option<&Value>) -> usize {
  value.and_then(Value::as_array).map_or(0, Vec::len)
}

fn sandbox_lines(report: &Value) -> Vec<String> {
  let sandbox = report.get("sandbox").unwrap_or(&Value::Null);
  let colima = report.get("colima").unwrap_or(&Value::Null);
  let mut out = vec![
    format!(
      "sandbox: {} {}",
      text(sandbox, "action"),
      text(sandbox, "reason")
    )
    .trim_end()
    .to_owned(),
  ];
  let (removed, kept) = (
    len(sandbox.pointer("/clones/removed")),
    len(sandbox.pointer("/clones/kept")),
  );
  if removed + kept > 0 {
    out.push(format!("  clones: {removed} orphan, {kept} kept"));
  }
  out.push(
    format!(
      "colima toolu-gate: {} {}",
      text(colima, "toolu-gate"),
      text(colima, "reason")
    )
    .trim_end()
    .to_owned(),
  );
  if let Some(dangling) = report.pointer("/docker/dangling").and_then(Value::as_u64) {
    let removed = report
      .pointer("/docker/removed")
      .map_or(String::new(), |n| format!(", {n} removed"));
    out.push(format!("docker: {dangling} dangling images{removed}"));
  }
  out
}

fn engine_lines(report: &Value) -> Vec<String> {
  let count = |key: &str| {
    report
      .pointer(&format!("/engine/{key}"))
      .and_then(Value::as_u64)
      .unwrap_or(0)
  };
  let mut out = vec![format!(
    "job logs: {} kept, {} removed",
    count("job_logs"),
    count("job_logs_removed")
  )];
  let rows = report
    .pointer("/engine/stale_records")
    .and_then(Value::as_array);
  for record in rows.into_iter().flatten() {
    let (key, stage, fix) = (
      text(record, "key"),
      text(record, "stage"),
      text(record, "fix"),
    );
    out.push(format!("stale record {key} ({stage}): {fix}"));
  }
  out
}

#[cfg(test)]
#[path = "tests/gc_test.rs"]
mod tests;
