//! Ledger step entries (`ledger-model.ts`): the `pending` seed, the `--step`
//! `running` pre-write, carry-forward of a prior entry with the plan's
//! authored fields re-applied, and a finished step that archives a prior red
//! attempt into `retries`.

use toolu_runtime::json::ordered::Ordered;

use super::jq::{JqError, NULL, alt, assign, get, is_str, length, number, string, type_name};

/// The authored fields a ledger entry re-derives from the plan on every run.
fn authored(step: &Ordered) -> Result<[(&'static str, Ordered); 4], JqError> {
  let empty = Ordered::Array(Vec::new());
  Ok([
    ("ac_refs", alt(get(step, "ac_refs")?, &empty).clone()),
    ("depends_on", alt(get(step, "depends_on")?, &empty).clone()),
    ("input", alt(get(step, "input")?, &NULL).clone()),
    ("model", alt(get(step, "model")?, &NULL).clone()),
  ])
}

fn entry(step: &Ordered, status: &str) -> Result<Vec<(String, Ordered)>, JqError> {
  let mut out = vec![
    ("id".to_owned(), get(step, "id")?.clone()),
    ("title".to_owned(), get(step, "title")?.clone()),
    ("check".to_owned(), get(step, "check")?.clone()),
    ("status".to_owned(), string(status)),
  ];
  for key in [
    "started_at",
    "activity",
    "exit_code",
    "diff_sha",
    "last_run",
    "evidence_tail",
  ] {
    out.push((key.to_owned(), Ordered::Null));
  }
  for (key, value) in authored(step)? {
    out.push((key.to_owned(), value));
  }
  Ok(out)
}

/// A never-run entry: the `pending` seed.
///
/// # Errors
/// [`JqError`] when the step is neither an object nor null.
pub fn pending_entry(step: &Ordered) -> Result<Ordered, JqError> {
  let mut out = entry(step, "pending")?;
  out.push(("retries".to_owned(), Ordered::Array(Vec::new())));
  Ok(Ordered::Object(out))
}

/// The `--step` pre-write entry: `running` since `now`.
///
/// # Errors
/// [`JqError`] when the step or the prior entry has a shape jq rejects.
pub fn running_entry(
  step: &Ordered,
  prior: &Ordered,
  now: &str,
  activity: &str,
) -> Result<Ordered, JqError> {
  let mut out = pending_entry(step)?;
  out.set("status", string("running"));
  out.set("started_at", string(now));
  out.set(
    "activity",
    if activity.is_empty() {
      Ordered::Null
    } else {
      string(activity)
    },
  );
  out.set("last_run", string(now));
  let empty = Ordered::Array(Vec::new());
  out.set("retries", alt(get(prior, "retries")?, &empty).clone());
  Ok(out)
}

/// The prior entry with the plan's authored fields re-applied (the `--step`
/// pre-write of a step that is not the target).
///
/// # Errors
/// [`JqError`] when the prior entry is not an object.
pub fn refresh_authored(prior: &Ordered, step: &Ordered) -> Result<Ordered, JqError> {
  let mut out = prior.clone();
  for (key, value) in authored(step)? {
    out = assign(&out, key, value)?;
  }
  Ok(out)
}

/// The prior entry carried forward with every engine field backfilled (a step
/// that is not the target, or a fresh step that is skipped).
///
/// # Errors
/// [`JqError`] when the prior entry is not an object.
pub fn carry_forward(prior: &Ordered, step: &Ordered) -> Result<Ordered, JqError> {
  let mut out = assign(
    prior,
    "scope_sha",
    alt(get(prior, "scope_sha")?, &NULL).clone(),
  )?;
  for (key, value) in authored(step)? {
    out = assign(&out, key, value)?;
  }
  out = assign(&out, "title", get(step, "title")?.clone())?;
  out = assign(&out, "check", get(step, "check")?.clone())?;
  let pending = string("pending");
  let status = alt(get(&out, "status")?, &pending).clone();
  out = assign(&out, "status", status)?;
  for key in [
    "started_at",
    "activity",
    "exit_code",
    "diff_sha",
    "last_run",
    "evidence_tail",
  ] {
    let value = alt(get(&out, key)?, &NULL).clone();
    out = assign(&out, key, value)?;
  }
  let empty = Ordered::Array(Vec::new());
  let retries = alt(get(&out, "retries")?, &empty).clone();
  assign(&out, "retries", retries)
}

/// How a step's check ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
  /// `green` or `red`.
  pub status: &'static str,
  /// The check's exit code.
  pub exit_code: i32,
  /// The branch hash the check ran against.
  pub sha: String,
  /// The tail of the check's output.
  pub evidence: String,
  /// When the check finished.
  pub now: String,
}

/// jq `a + b` for the retries list: arrays concatenate, a null left side is the right side.
fn append_retry(retries: &Ordered, record: Ordered) -> Result<Ordered, JqError> {
  match retries {
    Ordered::Null => Ok(Ordered::Array(vec![record])),
    Ordered::Array(items) => {
      let mut items = items.clone();
      items.push(record);
      Ok(Ordered::Array(items))
    }
    Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_) | Ordered::Object(_) => Err(
      JqError(format!("{} and array cannot be added", type_name(retries))),
    ),
  }
}

/// `pl_build_step_entry`: a finished step. A prior red entry is archived into
/// `retries`, and the prior's own retries carry forward.
///
/// # Errors
/// [`JqError`] when the step or the prior entry has a shape jq rejects.
pub fn build_step_entry(
  step: &Ordered,
  prior: &Ordered,
  run: &RunOutcome,
) -> Result<Ordered, JqError> {
  let empty = Ordered::Array(Vec::new());
  let prior_retries = alt(get(prior, "retries")?, &empty);
  let retries = if is_str(alt(get(prior, "status")?, &NULL), "red") {
    let record = Ordered::Object(vec![
      ("attempt".to_owned(), number(length(prior_retries)? + 1.0)),
      ("exit_code".to_owned(), get(prior, "exit_code")?.clone()),
      ("diff_sha".to_owned(), get(prior, "diff_sha")?.clone()),
      (
        "evidence_tail".to_owned(),
        get(prior, "evidence_tail")?.clone(),
      ),
      ("at".to_owned(), get(prior, "last_run")?.clone()),
    ]);
    append_retry(prior_retries, record)?
  } else {
    prior_retries.clone()
  };
  let mut out = entry(step, run.status)?;
  for (key, value) in [
    ("exit_code", number(f64::from(run.exit_code))),
    ("diff_sha", string(&run.sha)),
    ("last_run", string(&run.now)),
    ("evidence_tail", string(&run.evidence)),
  ] {
    if let Some(slot) = out.iter_mut().find(|(name, _)| name == key) {
      slot.1 = value;
    }
  }
  out.push(("retries".to_owned(), retries));
  Ok(Ordered::Object(out))
}

/// The number of retries an entry carries, plus one: the attempt telemetry reports.
pub fn attempt_of(entry: &Ordered) -> usize {
  match entry.get("retries") {
    Some(Ordered::Array(items)) => items.len() + 1,
    Some(
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Object(_),
    )
    | None => 1,
  }
}

#[cfg(test)]
#[path = "tests/entries_test.rs"]
mod tests;
