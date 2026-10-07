//! The four-gate verdict (`packages/toolu-core/src/ledger/verdict.ts`, #421):
//! one read-only view over the push gates (quality, plan, review and docs) that
//! answers "is this branch done, and why not". It never writes state or runs a
//! check. Exit 0 means ready, 1 blocked (a gate failed or escalated), 2 an error.

/// The docs gate.
pub mod docs;
/// The shared gate context and the quality gate.
pub mod gates;
/// Bash `case` globs.
pub mod glob;
/// The plan gate.
pub mod plan;
/// The review gate.
pub mod review;

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, current_branch, has_git, toplevel};
use toolu_state::time::iso_seconds;

use crate::ledger::io::{CommandResult, LedgerOptions, Output};
use crate::ledger::jq::{number, string};
use gates::{GateContext, quality_gate};

/// The gate names, in report order.
pub const GATES: [&str; 4] = ["quality", "plan", "review", "docs"];

/// Every gate for the repository at the invocation directory, and the config
/// warnings met on the way.
///
/// # Errors
/// `verdict: git is required`, or `verdict: not a git repository`.
pub fn verdict_report(opts: &LedgerOptions) -> Result<(Ordered, Vec<String>), String> {
  let env = opts.env();
  if !has_git(env) {
    return Err("verdict: git is required".to_owned());
  }
  let root = toplevel(env, &opts.cwd).ok_or_else(|| "verdict: not a git repository".to_owned())?;
  let branch = current_branch(env, &root);
  let base = match env.get("PUSH_REVIEW_BASE") {
    Some(base) => base.to_owned(),
    None => base_branch(env, Some(&root), &opts.cwd),
  };
  let cur = diff_sha(env, &root, &base).unwrap_or_default();
  let mut ctx = GateContext {
    roots: &opts.roots,
    root,
    branch,
    base,
    cur,
    cwd: opts.cwd.clone(),
    warnings: Vec::new(),
  };
  let gates = vec![
    ("quality".to_owned(), quality_gate(&ctx)),
    ("plan".to_owned(), plan::plan_gate(&ctx)),
    ("review".to_owned(), review::review_gate(&ctx)),
    ("docs".to_owned(), docs::docs_gate(&mut ctx)),
  ];
  let blocked = gates
    .iter()
    .any(|(_, gate)| matches!(gate.get("state"), Some(Ordered::String(state)) if state == "fail" || state == "escalate"));
  let report = Ordered::Object(vec![
    ("version".to_owned(), number(1.0)),
    ("branch".to_owned(), string(&ctx.branch)),
    ("base_branch".to_owned(), string(&ctx.base)),
    ("diff_sha".to_owned(), string(&ctx.cur)),
    (
      "overall".to_owned(),
      string(if blocked { "blocked" } else { "ready" }),
    ),
    ("gates".to_owned(), Ordered::Object(gates)),
    (
      "generated_at".to_owned(),
      string(&iso_seconds((opts.now)())),
    ),
  ]);
  Ok((report, ctx.warnings))
}

fn text(value: Option<&Ordered>) -> &str {
  match value {
    Some(Ordered::String(text)) => text,
    _ => "",
  }
}

/// `printf '%-8s %-9s %s\n'`.
fn row(gate: &str, state: &str, reason: &str) -> String {
  format!("{gate:<8} {state:<9} {reason}\n")
}

/// `vd_render_status`: the human table over the same data as `json`.
pub fn render_verdict_status(report: &Ordered) -> String {
  let sha = text(report.get("diff_sha"));
  let short: String = sha.chars().take(12).collect();
  let mut out = format!(
    "verdict: {} vs {} (diff {short})\n",
    text(report.get("branch")),
    text(report.get("base_branch"))
  );
  out.push_str(&row("GATE", "STATE", "REASON"));
  for name in GATES {
    let gate = report.get("gates").and_then(|gates| gates.get(name));
    let field = |key| text(gate.and_then(|gate| gate.get(key)));
    out.push_str(&row(name, field("state"), field("reason")));
  }
  out.push_str("overall: ");
  out.push_str(text(report.get("overall")));
  out.push('\n');
  out
}

/// `toolu ledger verdict status|json`: the table or the report; `json` is the
/// report's `jq .` bytes.
pub fn verdict_main(json: bool, opts: &LedgerOptions) -> CommandResult {
  let mut out = Output::default();
  match verdict_report(opts) {
    Err(line) => {
      out.stderr(line);
      out.result(2, None)
    }
    Ok((report, warnings)) => {
      for line in warnings {
        out.stderr(line);
      }
      let rendered = if json {
        format!("{}\n", jq_text(&report, true))
      } else {
        render_verdict_status(&report)
      };
      out.stdout(&rendered);
      let ready = text(report.get("overall")) == "ready";
      out.result(u8::from(!ready), Some(report))
    }
  }
}

#[cfg(test)]
#[path = "tests/verdict_test.rs"]
mod tests;
