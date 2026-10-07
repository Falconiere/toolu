//! The pure half of the ledger (`packages/toolu-core/src/ledger/ledger-model.ts`):
//! summary recompute, the summary line and orphan healing. Each function is a
//! direct port of its jq program, so it fails with `JqError` wherever jq
//! fails on a malformed ledger.

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use toolu_runtime::json::ordered::Ordered;
use toolu_state::time::iso_seconds;

use super::jq::{
  JqError, NULL, alt, assign, concat, count, each, equals, get, index, is_str, length, number,
  string, to_str, type_name,
};

/// `step.status == status`.
///
/// # Errors
/// [`JqError`] when `step` is neither an object nor null.
pub(crate) fn status_is(step: &Ordered, status: &str) -> Result<bool, JqError> {
  Ok(is_str(get(step, "status")?, status))
}

/// `is_fresh` from `pl_recompute`, with jq's short-circuiting `and`.
fn is_fresh(step: &Ordered, cur: &str, scope: &Ordered, verify: bool) -> Result<bool, JqError> {
  if !status_is(step, "green")? {
    return Ok(false);
  }
  if !verify {
    let scoped = alt(index(scope, get(step, "id")?)?, &NULL);
    if !matches!(scoped, Ordered::Null) {
      let own = get(step, "scope_sha")?;
      return Ok(!matches!(own, Ordered::Null) && equals(own, scoped));
    }
  }
  Ok(is_str(get(step, "diff_sha")?, cur))
}

fn count_where(
  steps: &[&Ordered],
  mut pred: impl FnMut(&Ordered) -> Result<bool, JqError>,
) -> Result<Ordered, JqError> {
  let mut n = 0;
  for step in steps {
    if pred(step)? {
      n += 1;
    }
  }
  Ok(number(count(n)))
}

/// `pl_recompute LEDGER CUR [SCOPE] [VERIFY]`: summary counts and `next`
/// against the current branch hash. A step with a declared scope (`scope`
/// maps its id to its scope hash) is judged on that hash unless `verify`.
///
/// # Errors
/// [`JqError`] where jq fails on the ledger's shape.
pub fn recompute(
  ledger: &Ordered,
  cur: &str,
  scope: &Ordered,
  verify: bool,
) -> Result<Ordered, JqError> {
  let steps = each(get(ledger, "steps")?)?;
  let fresh = |step: &Ordered| is_fresh(step, cur, scope, verify);
  let empty = Ordered::Array(Vec::new());
  let summary = Ordered::Object(vec![
    ("total".to_owned(), number(length(get(ledger, "steps")?)?)),
    (
      "green".to_owned(),
      count_where(&steps, |s| status_is(s, "green"))?,
    ),
    (
      "red".to_owned(),
      count_where(&steps, |s| status_is(s, "red"))?,
    ),
    (
      "pending".to_owned(),
      count_where(&steps, |s| status_is(s, "pending"))?,
    ),
    (
      "running".to_owned(),
      count_where(&steps, |s| status_is(s, "running"))?,
    ),
    (
      "stale".to_owned(),
      count_where(&steps, |s| Ok(status_is(s, "green")? && !fresh(s)?))?,
    ),
    ("fresh_green".to_owned(), count_where(&steps, fresh)?),
    (
      "retried".to_owned(),
      count_where(&steps, |s| {
        Ok(length(alt(get(s, "retries")?, &empty))? > 0.0)
      })?,
    ),
  ]);
  let with_summary = assign(ledger, "summary", summary)?;
  let mut next = Ordered::Null;
  for step in &steps {
    if !fresh(step)? {
      next = alt(get(step, "id")?, &NULL).clone();
      break;
    }
  }
  assign(&with_summary, "next", next)
}

/// `pl_summary_line`: `plan-ledger <slug>: <fresh>/<total> fresh-green, next=<id|none>[ model=<m>]`.
///
/// # Errors
/// [`JqError`] where jq fails on the ledger's shape.
pub fn summary_line(ledger: &Ordered, slug: &str) -> Result<String, JqError> {
  let next = get(ledger, "next")?;
  let mut model = &NULL;
  let mut matching = Vec::new();
  for step in each(get(ledger, "steps")?)? {
    if equals(get(step, "id")?, next) {
      matching.push(step);
    }
  }
  for step in matching {
    let candidate = get(step, "model")?;
    if !matches!(candidate, Ordered::Null) {
      model = candidate;
      break;
    }
  }
  let summary = get(ledger, "summary")?;
  let suffix = if matches!(model, Ordered::Null) {
    String::new()
  } else {
    concat(&[&string(" model="), model])?
  };
  let none = string("none");
  concat(&[
    &string(&format!("plan-ledger {slug}: ")),
    &string(&to_str(get(summary, "fresh_green")?)),
    &string("/"),
    &string(&to_str(get(summary, "total")?)),
    &string(" fresh-green, next="),
    alt(next, &none),
    &string(&suffix),
  ])
}

/// `pl_all_fresh`: every step fresh-green, i.e. no `next`.
pub fn all_fresh(ledger: &Ordered) -> bool {
  matches!(ledger.get("next"), None | Some(Ordered::Null))
}

/// `$steps[] | select(.id == $id)`: every matching step, duplicates included.
///
/// # Errors
/// [`JqError`] when a step is neither an object nor null.
pub fn steps_with_id<'a>(steps: &'a [Ordered], id: &Ordered) -> Result<Vec<&'a Ordered>, JqError> {
  let mut out = Vec::new();
  for step in steps {
    if equals(get(step, "id")?, id) {
      out.push(step);
    }
  }
  Ok(out)
}

/// jq's order between the orphan cutoff (a string) and a `started_at` of any type.
fn before_cutoff(started_at: &Ordered, cutoff: &str) -> bool {
  match started_at {
    Ordered::String(text) => text.as_str() < cutoff,
    Ordered::Null | Ordered::Bool(_) | Ordered::Number(_) => true,
    Ordered::Array(_) | Ordered::Object(_) => false,
  }
}

/// `now - threshold` seconds in the ledger's ISO format.
pub fn orphan_cutoff(now: SystemTime, threshold: i64) -> String {
  let secs = match now.duration_since(UNIX_EPOCH) {
    Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
    Err(_) => 0,
  };
  let at = secs.saturating_sub(threshold);
  let time = match u64::try_from(at) {
    Ok(after) => UNIX_EPOCH.checked_add(Duration::from_secs(after)),
    Err(_) => UNIX_EPOCH.checked_sub(Duration::from_secs(at.unsigned_abs())),
  };
  iso_seconds(time.unwrap_or(UNIX_EPOCH))
}

/// `pl_heal_orphans`: a `running` step whose `started_at` is missing, empty or
/// older than the cutoff goes back to `pending`.
///
/// # Errors
/// [`JqError`] where jq fails on the ledger's shape.
pub fn heal_orphans(ledger: &Ordered, cutoff: &str) -> Result<Ordered, JqError> {
  let mut healed = Vec::new();
  let empty = string("");
  for step in each(get(ledger, "steps")?)? {
    if !status_is(step, "running")? {
      healed.push(step.clone());
      continue;
    }
    let started_at = alt(get(step, "started_at")?, &empty);
    if !is_str(started_at, "") && !before_cutoff(started_at, cutoff) {
      healed.push(step.clone());
      continue;
    }
    let step = assign(step, "status", string("pending"))?;
    let step = assign(&step, "started_at", Ordered::Null)?;
    healed.push(assign(&step, "activity", Ordered::Null)?);
  }
  assign(ledger, "steps", Ordered::Array(healed))
}

/// jq `from_entries` over `{key: .id, value: .}`: the prior entries by id.
///
/// # Errors
/// [`JqError`] when a step's id is not a string.
pub fn entries_by_id(ledger: &Ordered) -> Result<HashMap<String, Ordered>, JqError> {
  let mut out = HashMap::new();
  for step in each(get(ledger, "steps")?)? {
    let id = get(step, "id")?;
    let Ordered::String(name) = id else {
      return Err(JqError(format!(
        "Cannot use {} as object key",
        type_name(id)
      )));
    };
    out.insert(name.clone(), step.clone());
  }
  Ok(out)
}

#[cfg(test)]
#[path = "tests/model_test.rs"]
mod tests;
