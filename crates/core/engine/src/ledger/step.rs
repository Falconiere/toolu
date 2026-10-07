//! One step's run (`runStep` in `packages/toolu-core/src/ledger/ledger-run.ts`):
//! its check into `<ledger>.run.<pid>.<id>`, the progress lines, the finished
//! entry with its scope hash, and the `step_run` telemetry.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use toolu_runtime::json::ordered::Ordered;
use toolu_state::ctx::StateCtx;
use toolu_state::telemetry::{TelemetryEvent, telemetry_append};

use super::check::{CheckRun, run_check, step_evidence};
use super::context::{CommandFail, RunContext, or_fail};
use super::entries::{RunOutcome, attempt_of, build_step_entry};
use super::io::Output;
use super::jq::{NULL, raw, string};

/// The prior entry of `id`, or null.
pub fn prior_of<'a>(ctx: &'a RunContext<'_>, id: &str) -> &'a Ordered {
  ctx.existing.get(id).unwrap_or(&NULL)
}

fn epoch_seconds(now: SystemTime) -> u64 {
  now
    .duration_since(UNIX_EPOCH)
    .map_or(0, |after| after.as_secs())
}

/// `<ledger>.run.<pid>.<id>`, the check's output file, with its directory made.
fn stage(ctx: &RunContext<'_>, id: &str) -> PathBuf {
  let mut tmp = ctx.ledger_file.as_os_str().to_owned();
  tmp.push(format!(".run.{}.{id}", std::process::id()));
  if let Some(dir) = ctx.ledger_file.parent() {
    // bash's `mkdir -p … || true`: the output redirect reports the real problem.
    let _made = std::fs::create_dir_all(dir);
  }
  PathBuf::from(tmp)
}

/// The check's exit code and combined output; a runner failure is exit 1 with no output.
fn execute(ctx: &RunContext<'_>, check: String, tmp: &Path) -> (i32, Vec<u8>) {
  let run = CheckRun {
    check,
    cwd: ctx.root.clone(),
    env: ctx.opts.env().clone(),
    out_file: tmp.to_path_buf(),
    timeout: ctx.timeout.clone(),
  };
  let result = run_check(&run).and_then(|code| {
    std::fs::read(tmp)
      .map(|output| (code, output))
      .map_err(|err| err.to_string())
  });
  let _removed = std::fs::remove_file(tmp);
  result.unwrap_or((1, Vec::new()))
}

fn telemetry(ctx: &RunContext<'_>, out: &mut Output, event: &TelemetryEvent) {
  let mut state = StateCtx::new(ctx.opts.roots.clone());
  state.now = Some(ctx.clock());
  let _result = telemetry_append(&mut state, &ctx.root, event);
  for line in state.warnings {
    out.stderr(line);
  }
}

/// The step a run is about: its id, its plan entries, its `[i/n]` and its scope hash.
pub struct Target<'a> {
  /// The step id.
  pub id: &'a str,
  /// Its plan entries (more than one when the id repeats).
  pub matches: &'a [&'a Ordered],
  /// `[i/n]`.
  pub at: &'a str,
  /// Its scope hash, or `""`.
  pub scope_now: &'a str,
}

/// The `step_run` telemetry event of a finished step.
fn step_event(id: &str, run: &RunOutcome, duration: u64, entry: &Ordered) -> TelemetryEvent {
  TelemetryEvent::StepRun {
    step_id: id.to_owned(),
    status: run.status.to_owned(),
    exit_code: f64::from(run.exit_code),
    duration_s: f64::from(u32::try_from(duration).unwrap_or(u32::MAX)),
    attempt: f64::from(u32::try_from(attempt_of(entry)).unwrap_or(u32::MAX)),
  }
}

/// The finished entries of `target`, each carrying the step's scope hash.
fn finished(
  ctx: &RunContext<'_>,
  target: &Target<'_>,
  run: &RunOutcome,
) -> Result<Vec<Ordered>, CommandFail> {
  let prior = prior_of(ctx, target.id);
  let scope_sha = if target.scope_now.is_empty() {
    Ordered::Null
  } else {
    string(target.scope_now)
  };
  let built = target
    .matches
    .iter()
    .map(|m| {
      build_step_entry(m, prior, run).map(|mut entry| {
        entry.set("scope_sha", scope_sha.clone());
        entry
      })
    })
    .collect::<Result<Vec<_>, _>>();
  or_fail(
    built,
    &format!("plan-ledger: failed to build entry for step {}", target.id),
  )
}

/// Run one step's check and build its finished entry (one per duplicate of `id`).
///
/// # Errors
/// `plan-ledger: failed to build entry for step <id>` when jq fails on an entry.
pub fn run_step(
  ctx: &RunContext<'_>,
  out: &mut Output,
  target: &Target<'_>,
) -> Result<Vec<Ordered>, CommandFail> {
  let Target {
    id, matches, at, ..
  } = *target;
  let tmp = stage(ctx, id);
  out.stderr(format!("plan-ledger: {at} {id}: running check"));
  let checks: Vec<String> = matches
    .iter()
    .map(|m| raw(m.get("check").unwrap_or(&NULL)))
    .collect();
  let started = epoch_seconds(SystemTime::now());
  let (code, output) = execute(
    ctx,
    checks.join("\n").trim_end_matches('\n').to_owned(),
    &tmp,
  );
  let duration = epoch_seconds(SystemTime::now()).saturating_sub(started);
  let status = if code == 0 { "green" } else { "red" };
  out.stderr(format!("plan-ledger: {at} {id}: {status} ({duration}s)"));
  let evidence = step_evidence(code, &output, &ctx.timeout);
  let run = RunOutcome {
    status,
    exit_code: code,
    sha: ctx.cur.clone(),
    evidence,
    now: ctx.now(),
  };
  let entries = finished(ctx, target, &run)?;
  // A duplicated id yields no single entry, and TypeScript then records nothing.
  if let [entry] = entries.as_slice() {
    telemetry(ctx, out, &step_event(id, &run, duration, entry));
  }
  Ok(entries)
}

#[cfg(test)]
#[path = "tests/step_test.rs"]
mod tests;
