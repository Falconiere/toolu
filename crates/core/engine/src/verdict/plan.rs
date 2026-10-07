//! The verdict's plan gate (`planGate` in `verdict-gates.ts`): the plan-ledger
//! push check recomputed against the current diff, with the report-only count
//! of spec ACs that no fresh-green step covers.

use std::path::PathBuf;

use toolu_runtime::json::ordered::Ordered;
use toolu_state::git::branch_slug;

use super::gates::{GateContext, field_or, gate, raw_or};
use crate::ledger::io::read_ledger;
use crate::ledger::jq::{
  JqError, NULL, alt, count, each, get, holds, is_str, length, number, string,
};
use crate::ledger::parse::{doc_field, is_file, is_specless, parse_acs, resolve};

const CODE_EXTENSIONS: [&str; 8] = ["ts", "tsx", "js", "jsx", "rs", "sh", "py", "go"];

fn zero_plan() -> Vec<(String, Ordered)> {
  let summary = Ordered::Object(vec![
    ("total".to_owned(), number(0.0)),
    ("fresh_green".to_owned(), number(0.0)),
  ]);
  vec![
    ("summary".to_owned(), summary),
    ("ac_uncovered".to_owned(), number(0.0)),
  ]
}

fn fresh_at(step: &Ordered, cur: &str) -> Result<bool, JqError> {
  Ok(is_str(get(step, "status")?, "green") && is_str(get(step, "diff_sha")?, cur))
}

/// `id: effective-status` for each step that is not fresh-green; a jq error keeps the lines before it.
fn blockers(ledger: &Ordered, cur: &str) -> Vec<String> {
  let mut lines = Vec::new();
  let Ok(steps) = get(ledger, "steps").and_then(each) else {
    return lines;
  };
  let (pending, unknown) = (string("pending"), string("?"));
  for step in steps {
    let Ok(fresh) = fresh_at(step, cur) else {
      return lines;
    };
    let green = get(step, "status").is_ok_and(|status| is_str(status, "green"));
    let effective = if fresh {
      continue;
    } else if green {
      string("stale")
    } else {
      alt(get(step, "status").unwrap_or(&NULL), &pending).clone()
    };
    let id = alt(get(step, "id").unwrap_or(&NULL), &unknown);
    match (id, &effective) {
      (Ordered::String(id), Ordered::String(state)) if state != "green" => {
        lines.push(format!("{id}: {state}"));
      }
      (_, Ordered::String(state)) if state == "green" => {}
      _ => return lines,
    }
  }
  lines
}

/// An absolute path as is; a relative one from cwd when it exists there, else under the root.
fn near(ctx: &GateContext<'_>, path: &str) -> PathBuf {
  let from_cwd = resolve(&ctx.cwd, path);
  if path.starts_with('/') || is_file(&from_cwd) {
    from_cwd
  } else {
    resolve(&ctx.root, path.trim_start_matches('/'))
  }
}

/// Whether a fresh-green step covers `ac`. jq builds the whole covering list
/// first, so a bad `ac_refs` anywhere fails the count.
fn covered(steps: &[&Ordered], ac: &str, cur: &str) -> Result<bool, JqError> {
  let empty = Ordered::Array(Vec::new());
  let mut covering = Vec::new();
  for step in steps {
    if holds(alt(get(step, "ac_refs")?, &empty), ac)? {
      covering.push(*step);
    }
  }
  for step in covering {
    if fresh_at(step, cur)? {
      return Ok(true);
    }
  }
  Ok(false)
}

/// The report-only count of spec ACs with no fresh-green covering step.
fn ac_uncovered(ctx: &GateContext<'_>, ledger: &Ordered) -> usize {
  let plan_doc = field_or(ledger, "plan_doc", "");
  if plan_doc.is_empty() || !is_file(&near(ctx, &plan_doc)) {
    return 0;
  }
  let spec_field = doc_field(&near(ctx, &plan_doc), "Spec");
  if is_specless(&spec_field) || !is_file(&near(ctx, &spec_field)) {
    return 0;
  }
  let count_uncovered = || -> Result<usize, JqError> {
    let steps = each(get(ledger, "steps")?)?;
    let mut uncovered = 0;
    for ac in parse_acs(&near(ctx, &spec_field)) {
      uncovered += usize::from(!covered(&steps, &ac, &ctx.cur)?);
    }
    Ok(uncovered)
  };
  count_uncovered().unwrap_or(0)
}

/// The fresh count, the step total and the uncovered ACs; `{}` when jq fails on the steps.
fn extra(ctx: &GateContext<'_>, ledger: &Ordered) -> Vec<(String, Ordered)> {
  let summary = || -> Result<Ordered, JqError> {
    let mut fresh = 0;
    for step in each(get(ledger, "steps")?)? {
      fresh += usize::from(fresh_at(step, &ctx.cur)?);
    }
    Ok(Ordered::Object(vec![
      ("total".to_owned(), number(length(get(ledger, "steps")?)?)),
      ("fresh_green".to_owned(), number(count(fresh))),
    ]))
  };
  match summary() {
    Ok(summary) => vec![
      ("summary".to_owned(), summary),
      (
        "ac_uncovered".to_owned(),
        number(count(ac_uncovered(ctx, ledger))),
      ),
    ],
    Err(_) => Vec::new(),
  }
}

fn no_ledger(ctx: &GateContext<'_>) -> Ordered {
  // A failed diff reads as no code files; an unresolvable base also empties the
  // branch hash, and the review gate then names it.
  let range = format!("{}...HEAD", ctx.base);
  let names = ctx
    .git(&["diff", "--no-color", &range, "--name-only"])
    .unwrap_or_default();
  let code = names.split('\n').any(|name| {
    name
      .rsplit_once('.')
      .is_some_and(|(_, ext)| CODE_EXTENSIONS.contains(&ext))
  });
  if code {
    gate(
      "advise",
      "no plan ledger for this change; if non-trivial, run plan",
      zero_plan(),
    )
  } else {
    gate(
      "skip",
      "no plan ledger and no code files in diff",
      zero_plan(),
    )
  }
}

/// `vd_gate_plan`: the plan-ledger push check, recomputed against the current diff.
pub fn plan_gate(ctx: &GateContext<'_>) -> Ordered {
  let dir = ctx.state_dir("plan-ledger", "LEDGER_DIR");
  let file = dir.join(format!("{}.json", branch_slug(&ctx.branch)));
  if !is_file(&file) {
    return no_ledger(ctx);
  }
  let Some(read) = read_ledger(&file) else {
    return gate(
      "fail",
      &format!("unparseable ledger at {}", file.display()),
      zero_plan(),
    );
  };
  let ledger = read.value;
  let version = field_or(&ledger, "version", "");
  if version != "1" {
    let reason = format!(
      "ledger schema mismatch at {} (version=\"{version}\", expected 1)",
      file.display()
    );
    return gate("fail", &reason, zero_plan());
  }
  let zero = number(0.0);
  let total = raw_or(
    get(&ledger, "summary")
      .and_then(|s| get(s, "total"))
      .map(|t| alt(t, &zero)),
    "0",
  );
  let steps = length(get(&ledger, "steps").unwrap_or(&NULL))
    .map_or_else(|_| "0".to_owned(), |n| raw_or(Ok(&number(n)), "0"));
  if total == "0" || steps == "0" {
    return gate("skip", "empty plan ledger", zero_plan());
  }
  let extra = extra(ctx, &ledger);
  let lines = blockers(&ledger, &ctx.cur);
  if lines.is_empty() {
    gate("pass", "all plan-ledger steps fresh-green", extra)
  } else {
    gate(
      "fail",
      &format!("steps not fresh-green: {}", lines.join(",")),
      extra,
    )
  }
}

#[cfg(test)]
#[path = "tests/plan_test.rs"]
mod tests;
