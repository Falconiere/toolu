//! `toolu ledger run` (`packages/toolu-core/src/ledger/ledger-run.ts`): parse
//! the plan, run each step's check, stamp the mechanical status and the
//! content-addressed hashes, and write the per-branch ledger. Exit 0 when every
//! step is fresh-green, 1 otherwise, and 2 on a parse or I/O error, when
//! nothing is written. The check's exit code decides green, never the agent.

use toolu_runtime::json::ordered::Ordered;
use toolu_state::git::branch_slug;

use super::context::{CommandFail, RunContext, RunFlags, or_fail, prepare};
use super::entries::{carry_forward, pending_entry, refresh_authored, running_entry};
use super::io::{CommandResult, LedgerOptions, Output};
use super::jq::{NULL, alt, equals, get, raw, string};
use super::model::{all_fresh, recompute, steps_with_id, summary_line};
use super::scope::scope_map;
use super::step::{Target, prior_of, run_step};

fn step_id(step: &Ordered) -> String {
  raw(step.get("id").unwrap_or(&NULL))
}

/// `--step` write #1: the target step marked `running` before its check starts.
fn pre_write(ctx: &RunContext<'_>) -> Result<(), CommandFail> {
  let started_at = ctx.now();
  let assemble = || {
    let mut steps = Vec::new();
    for step in &ctx.steps {
      let id = step_id(step);
      let prior = ctx.existing.get(&id);
      for found in steps_with_id(&ctx.steps, step.get("id").unwrap_or(&NULL))? {
        steps.push(match prior {
          _ if id == ctx.flags.step() => running_entry(
            found,
            prior.unwrap_or(&NULL),
            &started_at,
            ctx.flags.activity(),
          )?,
          None => pending_entry(found)?,
          Some(prior) => refresh_authored(prior, found)?,
        });
      }
    }
    Ok(steps)
  };
  let steps = or_fail(
    assemble(),
    "plan-ledger: failed to assemble running pre-write",
  )?;
  // TypeScript reads the scope map before assigning it: the pre-write is judged without scopes.
  let empty = Ordered::Object(Vec::new());
  let ledger = or_fail(
    recompute(
      &ctx.ledger_doc(&started_at, steps),
      &ctx.cur,
      &empty,
      ctx.flags.verify,
    ),
    "plan-ledger: failed to recompute running pre-ledger",
  )?;
  ctx.write_or_fail(&ledger, "plan-ledger: running pre-write failed")
}

/// A step already fresh-green at its key is carried forward; anything else runs.
fn run_or_skip(
  ctx: &RunContext<'_>,
  out: &mut Output,
  id: &str,
  matches: &[&Ordered],
  at: &str,
) -> Result<Vec<Ordered>, CommandFail> {
  let prior = prior_of(ctx, id);
  let scope_now = match ctx.scope.get(id) {
    Some(Ordered::String(sha)) => sha.clone(),
    _ => String::new(),
  };
  let use_scope = !ctx.flags.verify && !scope_now.is_empty();
  let empty = string("");
  let key_field = if use_scope { "scope_sha" } else { "diff_sha" };
  let prior_key = raw(alt(get(prior, key_field).unwrap_or(&NULL), &empty));
  let now_key = if use_scope {
    scope_now.clone()
  } else {
    ctx.cur.clone()
  };
  let green = raw(alt(get(prior, "status").unwrap_or(&NULL), &empty)) == "green";
  if !ctx.flags.force
    && ctx.flags.step().is_empty()
    && green
    && !now_key.is_empty()
    && prior_key == now_key
  {
    out.stderr(format!(
      "plan-ledger: {at} {id}: fresh-green, skipped (--force re-runs)"
    ));
    let carried = matches.iter().map(|m| carry_forward(prior, m)).collect();
    return or_fail(
      carried,
      &format!("plan-ledger: failed to carry forward step {id}"),
    );
  }
  run_step(
    ctx,
    out,
    &Target {
      id,
      matches,
      at,
      scope_now: &scope_now,
    },
  )
}

fn run_steps(ctx: &RunContext<'_>, out: &mut Output) -> Result<Vec<Ordered>, CommandFail> {
  let mut entries_out = Vec::new();
  let mut index = 0;
  let total = ctx.steps.len();
  for step in &ctx.steps {
    let id = step_id(step);
    let matches = steps_with_id(&ctx.steps, step.get("id").unwrap_or(&NULL)).unwrap_or_default();
    let entries = if !ctx.flags.step().is_empty() && id != ctx.flags.step() {
      let prior = ctx.existing.get(&id);
      let assembled = matches
        .iter()
        .map(|m| prior.map_or_else(|| pending_entry(m), |prior| carry_forward(prior, m)))
        .collect();
      or_fail(
        assembled,
        &format!("plan-ledger: failed to assemble entry for step {id}"),
      )?
    } else {
      index += 1;
      run_or_skip(ctx, out, &id, &matches, &format!("[{index}/{total}]"))?
    };
    match <[Ordered; 1]>::try_from(entries) {
      Ok([entry]) => entries_out.push(entry),
      Err(_) => {
        return Err(CommandFail::line(format!(
          "plan-ledger: failed to append step {id}"
        )));
      }
    }
  }
  Ok(entries_out)
}

/// The prior ledger's `verified_sha` as `jq -r '.verified_sha // ""'` reads it.
fn prior_verified(ctx: &RunContext<'_>) -> String {
  let empty = string("");
  ctx
    .prior
    .as_ref()
    .and_then(|prior| get(&prior.value, "verified_sha").ok())
    .map(|sha| raw(alt(sha, &empty)))
    .unwrap_or_default()
}

/// Assemble, recompute, stamp `verified_sha`, write, and print the summary line.
fn finish(
  ctx: &RunContext<'_>,
  out: &mut Output,
  steps: Vec<Ordered>,
) -> Result<Ordered, CommandFail> {
  let failed = "plan-ledger: failed to recompute summary";
  let mut ledger = or_fail(
    recompute(
      &ctx.ledger_doc(&ctx.now(), steps),
      &ctx.cur,
      &ctx.scope,
      ctx.flags.verify,
    ),
    failed,
  )?;
  let summary = ledger.get("summary").unwrap_or(&NULL);
  let verified = ctx.flags.verify
    && ctx.flags.step().is_empty()
    && equals(
      summary.get("fresh_green").unwrap_or(&NULL),
      summary.get("total").unwrap_or(&NULL),
    );
  let previous = prior_verified(ctx);
  let stamp = match (verified, previous.is_empty()) {
    (true, _) => string(&ctx.cur),
    (false, true) => Ordered::Null,
    (false, false) => string(&previous),
  };
  ledger.set("verified_sha", stamp);
  ctx.write_or_fail(&ledger, "plan-ledger: ledger write failed")?;
  // TypeScript prints nothing when jq cannot render the line, and exits on freshness alone.
  if let Ok(line) = summary_line(&ledger, &branch_slug(&ctx.branch)) {
    out.stdout(&format!("{line}\n"));
  }
  Ok(ledger)
}

fn run_inner(
  doc: &str,
  flags: &RunFlags,
  opts: &LedgerOptions,
  out: &mut Output,
) -> Result<Ordered, CommandFail> {
  flags.check()?;
  let mut ctx = prepare(doc, flags, opts)?;
  if !flags.step().is_empty() {
    pre_write(&ctx)?;
  }
  let mut warnings = Vec::new();
  let scope = scope_map(&ctx.steps, &ctx.base, &ctx.root, opts.env(), &mut |line| {
    warnings.push(line);
  });
  for line in warnings {
    out.stderr(line);
  }
  ctx.scope = scope.unwrap_or_else(|_| Ordered::Object(Vec::new()));
  let steps = run_steps(&ctx, out)?;
  finish(&ctx, out, steps)
}

/// `toolu ledger run <doc> [--step <id>] [--activity <label>] [--force] [--verify]`.
pub fn ledger_run(doc: &str, flags: &RunFlags, opts: &LedgerOptions) -> CommandResult {
  let mut out = Output::default();
  match run_inner(doc, flags, opts, &mut out) {
    Ok(ledger) => {
      let exit = u8::from(!all_fresh(&ledger));
      out.result(exit, Some(ledger))
    }
    Err(CommandFail(lines)) => {
      for line in lines {
        out.stderr(line);
      }
      out.result(2, None)
    }
  }
}

#[cfg(test)]
#[path = "tests/run_test.rs"]
mod tests;
