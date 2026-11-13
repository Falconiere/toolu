//! Detached `toolu ledger run`: one native copy owns the blocking check and
//! records its result under the shared resource root.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use toolu_engine::ledger::context::RunFlags;
use toolu_engine::ledger::io::{LedgerOptions, project_root};
use toolu_engine::ledger::run::ledger_run_guarded;
use toolu_engine::resources::binding::resource_binding;
use toolu_engine::resources::job_results;
use toolu_engine::resources::store::resource_home;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::invocation::current_exe;
use toolu_runtime::process::{Spec, spawn_orphan};
use toolu_state::time::iso_millis;
use toolu_state::tree_sha::tree_sha;

use crate::ledger::outcome::ran;

fn location(opts: &LedgerOptions) -> Result<(PathBuf, String, PathBuf), Outcome> {
  let worktree = project_root(opts)
    .ok_or_else(|| Outcome::failed(Exit::Blocked, "plan-ledger: not in a git repo".to_owned()))?;
  let binding = resource_binding(&opts.cwd).map_err(|err| Outcome::failed(Exit::Failure, err))?;
  Ok(match binding {
    Some(binding) => (binding.root, binding.key, binding.worktree),
    None => (resource_home(opts.env()), String::new(), worktree),
  })
}

fn fail_early(root: &Path, record: &mut job_results::Record, message: String) -> Outcome {
  Outcome::failed(Exit::Failure, job_results::fail(root, record, message))
}

fn relaunch(exe: &Path, id: &str, doc: &str, flags: &RunFlags, ctx: &Ctx) -> Vec<String> {
  let mut args = vec![exe.display().to_string()];
  if ctx.json {
    args.push("--json".to_owned());
  }
  if ctx.quiet {
    args.push("--quiet".to_owned());
  }
  if let Some(host) = ctx.host {
    args.extend(["--host".to_owned(), host.name().to_owned()]);
  }
  if let Some(dir) = &ctx.config_dir {
    args.extend(["--config-dir".to_owned(), dir.display().to_string()]);
  }
  args.extend([
    "ledger".to_owned(),
    "run".to_owned(),
    doc.to_owned(),
    "--result".to_owned(),
    id.to_owned(),
  ]);
  if let Some(step) = &flags.only_step {
    args.extend(["--step".to_owned(), step.clone()]);
  }
  if let Some(activity) = &flags.activity {
    args.extend(["--activity".to_owned(), activity.clone()]);
  }
  if flags.force {
    args.push("--force".to_owned());
  }
  if flags.verify {
    args.push("--verify".to_owned());
  }
  args
}

/// Create a waiting result, relaunch a copy, and return its id without waiting.
pub(crate) fn run_detached(
  doc: &str,
  flags: &RunFlags,
  opts: &LedgerOptions,
  ctx: &Ctx,
) -> Outcome {
  let (root, key, worktree) = match location(opts) {
    Ok(found) => found,
    Err(outcome) => return outcome,
  };
  let mut record = match job_results::create(&root, &key, &worktree, &format!("ledger run {doc}")) {
    Ok(record) => record,
    Err(err) => return Outcome::failed(Exit::Failure, err),
  };
  let Some(exe) = current_exe() else {
    return fail_early(
      &root,
      &mut record,
      "cannot find the current executable".to_owned(),
    );
  };
  let mut spec = Spec::new(relaunch(&exe, &record.id, doc, flags, ctx));
  spec.cwd = Some(opts.cwd.clone());
  match spawn_orphan(&spec) {
    Ok(_) => Outcome::data(format!("{}\n", record.id)),
    Err(err) => fail_early(
      &root,
      &mut record,
      format!("toolu ledger run: cannot detach: {err:?}"),
    ),
  }
}

fn printed(outcome: &Outcome) -> String {
  let mut text = String::new();
  for part in [&outcome.stdout, &outcome.stderr].into_iter().flatten() {
    text.push_str(part);
    if !part.ends_with('\n') {
      text.push('\n');
    }
  }
  text
}

/// Run the existing check path, record its outcome, and notify the engine.
pub(crate) fn run_result(
  id: &str,
  doc: &str,
  flags: &RunFlags,
  opts: &LedgerOptions,
  ctx: &Ctx,
) -> Outcome {
  let (root, _, worktree) = match location(opts) {
    Ok(found) => found,
    Err(outcome) => return outcome,
  };
  let mut record = match job_results::get(&root, id) {
    Ok(Some(record)) => record,
    Ok(None) => return Outcome::failed(Exit::Failure, format!("unknown job result {id}")),
    Err(err) => return Outcome::failed(Exit::Failure, err),
  };
  if record.worktree != worktree.display().to_string() {
    return Outcome::failed(
      Exit::Failure,
      "job result belongs to another worktree".to_owned(),
    );
  }
  "running".clone_into(&mut record.state);
  if let Err(err) = job_results::update(&root, &record) {
    return fail_early(
      &root,
      &mut record,
      format!("job result not recorded: {err}"),
    );
  }
  let Some(initial) = tree_sha(opts.env(), &worktree) else {
    return fail_early(
      &root,
      &mut record,
      "plan-ledger: cannot hash working tree".to_owned(),
    );
  };
  let telemetry_dir = root.join("jobs/telemetry").join(id);
  let (result, stale) = ledger_run_guarded(doc, flags, opts, &initial, &telemetry_dir);
  let outcome = ran(&result, ctx.json);
  "done".clone_into(&mut record.state);
  record.exit_code = Some(i64::from(outcome.exit.code()));
  record.finished_at = Some(iso_millis(SystemTime::now()));
  record.output = printed(&outcome);
  record.stale = stale;
  if let Err(err) = job_results::update(&root, &record) {
    return Outcome::failed(Exit::Failure, format!("job result not recorded: {err}"));
  }
  job_results::announce(&root, &record.key);
  outcome
}

#[cfg(test)]
#[path = "tests/ledger_detach_test.rs"]
mod tests;
