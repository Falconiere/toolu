//! `toolu ledger preflight [<doc>]` (`ledgerPreflight` in `ledger-commands.ts`):
//! exit 0 only when the plan is Approved and its declared spec, if any, is
//! Approved. It also heals orphaned `running` steps in the branch ledger, and
//! without a doc it checks the ledger's own `plan_doc`.

use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;
use toolu_runtime::json::jq_text;

use super::commands::{cutoff, node_join};
use super::io::{
  CommandResult, LedgerOptions, Output, ledger_path, project_root, read_ledger, write_ledger,
};
use super::jq::{alt, get, raw, string};
use super::model::heal_orphans;
use super::parse::{doc_field, is_specless, resolve};

/// Heal the branch ledger in place when that changes it; its `plan_doc`, or `""`.
fn heal_ledger_for(opts: &LedgerOptions) -> String {
  let Some(file) = ledger_path(opts) else {
    return String::new();
  };
  let Some(read) = read_ledger(&file) else {
    return String::new();
  };
  if let Ok(healed) = heal_orphans(&read.value, &cutoff(opts))
    && jq_text(&healed, true) != read.text
  {
    let _written = write_ledger(&file, &healed);
  }
  let empty = string("");
  get(&read.value, "plan_doc")
    .map(|doc| raw(alt(doc, &empty)))
    .unwrap_or_default()
}

/// Where a refused preflight sends the agent: `OpenCode` loads the generated skill by its id.
fn review_remedy(host: Host, phase: &str) -> String {
  let action = match host {
    Host::Opencode => "load skill({ name: \"delivery-flow-delivery-flow\" })",
    Host::Claude | Host::Codex => "run /delivery-flow:delivery-flow",
    Host::Cursor | Host::Hermes => "load the delivery-flow skill",
  };
  format!("{action} ({phase} review phase)")
}

/// bash `[ -r PATH ]`.
fn readable(path: &Path) -> bool {
  std::fs::File::open(path).is_ok()
}

fn is_approved(status: &str) -> bool {
  status.eq_ignore_ascii_case("approved")
}

/// The declared spec's verdict, once the plan is Approved.
fn spec_check(
  plan_abs: &Path,
  host: Host,
  under: &dyn Fn(&str) -> PathBuf,
) -> (u8, Option<String>) {
  let spec = doc_field(plan_abs, "Spec");
  if is_specless(&spec) {
    return (0, None);
  }
  let spec_abs = under(&spec);
  if !readable(&spec_abs) {
    return (
      1,
      Some(format!(
        "preflight: declared spec not found or unreadable: {spec}"
      )),
    );
  }
  let spec_status = doc_field(&spec_abs, "Status");
  if is_approved(&spec_status) {
    return (0, None);
  }
  let shown = if spec_status.is_empty() {
    "none"
  } else {
    spec_status.as_str()
  };
  let remedy = review_remedy(host, "spec");
  (
    1,
    Some(format!(
      "preflight: spec {spec} not approved (Status: {shown}) — {remedy}"
    )),
  )
}

/// The preflight verdict: an exit code and the line explaining a refusal.
fn checks(plan: &str, root: Option<&Path>, cwd: &Path, host: Host) -> (u8, Option<String>) {
  let under = |path: &str| -> PathBuf {
    match root {
      Some(root) if !path.starts_with('/') => node_join(root, path),
      _ => resolve(cwd, path),
    }
  };
  let plan_abs = under(plan);
  if !readable(&plan_abs) {
    return (
      2,
      Some(format!(
        "preflight: plan doc not found or unreadable: {plan}"
      )),
    );
  }
  let status = doc_field(&plan_abs, "Status");
  let plan_review = review_remedy(host, "plan");
  if status.is_empty() {
    let line =
      format!("preflight: plan has no **Status:** header ({plan}) — {plan_review} to stamp it");
    return (1, Some(line));
  }
  if !is_approved(&status) {
    return (
      1,
      Some(format!(
        "preflight: plan not approved (Status: {status}) — {plan_review}"
      )),
    );
  }
  spec_check(&plan_abs, host, &under)
}

/// `toolu ledger preflight [<doc>]`.
pub fn ledger_preflight(plan: Option<&str>, opts: &LedgerOptions) -> CommandResult {
  let mut out = Output::default();
  let root = project_root(opts);
  let from_ledger = heal_ledger_for(opts);
  let target = plan
    .filter(|plan| !plan.is_empty())
    .map_or(from_ledger, str::to_owned);
  if target.is_empty() {
    out.stderr("preflight: no plan doc given and no ledger plan_doc to resolve");
    return out.result(2, None);
  }
  let (exit, line) = checks(&target, root.as_deref(), &opts.cwd, opts.roots.host());
  if let Some(line) = line {
    out.stderr(line);
  }
  out.result(exit, None)
}

#[cfg(test)]
#[path = "tests/preflight_test.rs"]
mod tests;
