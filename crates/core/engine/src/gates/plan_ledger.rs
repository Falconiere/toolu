//! Push-time plan ledger gate: no step runs here; every step must be fresh-green.

use std::path::{Path, PathBuf};

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, branch_slug};

use super::decided;
use super::plan_ledger_ac::{AcCheck, ac_blockers};
use super::push_target::{PushTarget, active_push, git_at};
use crate::ledger::io::read_ledger;
use crate::ledger::jq::{JqError, alt, concat, each, get, is_str, length, number, raw, string};
use crate::ledger::parse::is_file;
use crate::verdict::gates::{field_or, raw_or};

/// The plan-ledger built-in.
pub(crate) struct PlanLedger;
/// Its singleton in the pre-tool table.
pub(crate) static PLAN_LEDGER: PlanLedger = PlanLedger;

const RUN: &str = "toolu ledger run";

fn blocker_line(step: &Ordered, current: &str) -> Result<Option<String>, JqError> {
  let status = get(step, "status")?;
  let green = is_str(status, "green");
  let fresh = green && is_str(get(step, "diff_sha")?, current);
  if fresh {
    return Ok(None);
  }
  let pending = string("pending");
  let effective = if green {
    string("stale")
  } else {
    alt(status, &pending).clone()
  };
  if is_str(&effective, "green") {
    return Ok(None);
  }
  let unknown = string("?");
  let empty = string("");
  let colon = string(": ");
  let dash = string(" — ");
  let text = concat(&[
    alt(get(step, "id")?, &unknown),
    &colon,
    &effective,
    &dash,
    alt(get(step, "title")?, &empty),
  ])?;
  Ok(Some(text))
}

fn blockers(ledger: &Ordered, current: &str) -> String {
  let Ok(steps) = get(ledger, "steps").and_then(each) else {
    return String::new();
  };
  let mut lines = Vec::new();
  for step in steps {
    match blocker_line(step, current) {
      Ok(Some(line)) => lines.push(line),
      Ok(None) => {}
      Err(_) => break,
    }
  }
  lines.join("\n").trim_end_matches('\n').to_owned()
}

fn steps_decision(
  mode: GateMode,
  ledger: &Ordered,
  current: &str,
  hint: &str,
) -> Result<Decision, String> {
  let blocked = blockers(ledger, current);
  let empty = string("");
  let verified = raw_or(
    get(ledger, "verified_sha").map(|value| alt(value, &empty)),
    "",
  );
  if blocked.is_empty() && ledger.get("verified_sha").is_some() && verified != current {
    return decided(
      mode,
      format!(
        "plan-ledger: every step is green, but not verified against the current diff.\n\nScoped runs judge a step on its own `paths`; the push gate judges it on the whole branch.\n\nrun: {RUN} {hint} --verify"
      ),
    );
  }
  if blocked.is_empty() {
    return Ok(Decision::Allow);
  }
  decided(
    mode,
    format!("plan-ledger: push blocked — steps not fresh-green:\n{blocked}\n\nrun: {RUN} {hint}"),
  )
}

struct Check<'a> {
  ctx: &'a RuleContext<'a>,
  config: &'a toolu_runtime::config::load::LoadedConfig,
  state: &'a mut StateCtx,
  mode: GateMode,
  root: &'a Path,
  base: &'a str,
  file: &'a Path,
}

fn schema_or_empty(
  ledger: &Ordered,
  file: &Path,
  mode: GateMode,
) -> Result<Option<Decision>, String> {
  let version = field_or(ledger, "version", "");
  if version != "1" {
    return decided(
      mode,
      format!(
        "plan-ledger: ledger schema mismatch at {} (version=\"{version}\", expected 1); delete and re-run `{RUN} <plan_doc>`",
        file.display()
      ),
    )
    .map(Some);
  }
  let zero = number(0.0);
  let total = raw_or(
    get(ledger, "summary")
      .and_then(|sum| get(sum, "total"))
      .map(|value| alt(value, &zero)),
    "0",
  );
  let count = get(ledger, "steps")
    .and_then(length)
    .map_or_else(|_| "0".to_owned(), |value| raw(&number(value)));
  Ok((total == "0" || count == "0").then_some(Decision::Allow))
}

fn check_ledger(check: &mut Check<'_>) -> Result<Decision, String> {
  let Check {
    ctx,
    config,
    state,
    mode,
    root,
    base,
    file,
  } = check;
  if !is_file(file) {
    return Ok(Decision::Allow);
  }
  let Some(read) = read_ledger(file) else {
    let reason = Text::new(format!(
      "plan-ledger: unparseable ledger at {}; delete and re-run `{RUN} <plan_doc>`",
      file.display()
    ))
    .map_err(|err| err.to_string())?;
    return Ok(Decision::Deny { reason });
  };
  let ledger = read.value;
  if let Some(decision) = schema_or_empty(&ledger, file, *mode)? {
    return Ok(decision);
  }
  let Some(current) = diff_sha(ctx.env, root, base) else {
    return Ok(Decision::Allow);
  };
  let mut ac = AcCheck {
    ledger: &ledger,
    current: &current,
    root,
    ctx,
    config,
    state,
  };
  if let Some(uncovered) = ac_blockers(&mut ac) {
    return decided(
      *mode,
      format!(
        "plan-ledger: push blocked — uncovered spec AC id(s):\n{uncovered}\n\ncover with a fresh-green step referencing it in ac_refs, or set planLedger.blockOnUncoveredAcs=false"
      ),
    );
  }
  let hint = field_or(&ledger, "plan_doc", "<plan_doc>");
  steps_decision(*mode, &ledger, &current, &hint)
}

fn ledger_file(ctx: &RuleContext<'_>, roots: &Roots, root: &Path, branch: &str) -> PathBuf {
  let branch = if branch.is_empty() {
    git_at(root, &["rev-parse", "--abbrev-ref", "HEAD"], ctx.env)
      .unwrap_or_default()
      .trim()
      .to_owned()
  } else {
    branch.to_owned()
  };
  let dir = ctx.env.get("LEDGER_DIR").map_or_else(
    || {
      roots
        .project_state_dir("plan-ledger", None, Some(root))
        .ok()
        .flatten()
        .unwrap_or_default()
        .display()
        .to_string()
    },
    str::to_owned,
  );
  PathBuf::from(format!("{dir}/{}.json", branch_slug(&branch)))
}

fn evaluate(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  warnings: &mut Vec<String>,
) -> Result<Decision, String> {
  let Some((PushTarget { root, branch }, config, mode)) =
    active_push(event, ctx, "planLedger", warnings)
  else {
    return Ok(Decision::Allow);
  };
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let base = ctx
    .env
    .get("PUSH_REVIEW_BASE")
    .map_or_else(|| base_branch(ctx.env, Some(&root), &root), str::to_owned);
  let file = ledger_file(ctx, &roots, &root, &branch);
  let mut state = StateCtx::new(roots);
  let mut check = Check {
    ctx,
    config: &config,
    state: &mut state,
    mode,
    root: &root,
    base: &base,
    file: &file,
  };
  let result = check_ledger(&mut check);
  warnings.append(&mut state.warnings);
  result
}

impl_pre_gate!(PlanLedger, "plan-ledger", evaluate);

#[cfg(test)]
#[path = "tests/plan_ledger_test.rs"]
mod tests;
