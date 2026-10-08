//! Require a clean review of the exact diff a `git push` would send.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, branch_slug};
use toolu_state::telemetry::{TelemetryEvent, telemetry_append};

use super::push_review_state::{state_failure, state_round};
use super::push_target::{PushTarget, changed_names, push_target, ref_exists};
use super::{decided, gate_config, pre_mode};
use crate::gate::Gate;
use crate::verdict::gates::read_json;
use crate::waiver::Waivers;

/// The review gate in the pre-tool table.
pub(crate) struct PushReview;
/// Its singleton.
pub(crate) static PUSH_REVIEW: PushReview = PushReview;

const EMPTY_BLOB_SHA: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
const ASK_LEAD: &str = "No clean review is recorded for this diff. Approving pushes anyway, and the approval is remembered until the diff changes.";
const HINT: &str = "the built-in `/code-review xhigh --fix` skill, recorded as \"code-review\" (or the `toolu-review:review` skill)";

struct Push<'a> {
  ctx: &'a RuleContext<'a>,
  mode: GateMode,
  root: PathBuf,
  slug: String,
  base: String,
  state: StateCtx,
}

impl Push<'_> {
  fn record(&mut self, result: &str, code: &str, round: &str) {
    let parsed = round
      .bytes()
      .all(|byte| byte.is_ascii_digit())
      .then(|| round.parse::<f64>().ok())
      .flatten();
    let event = TelemetryEvent::PushCheck {
      result: result.to_owned(),
      reason_code: code.to_owned(),
      round: parsed,
    };
    let _written = telemetry_append(&mut self.state, &self.root, &event);
  }

  fn decide(
    &mut self,
    code: &str,
    reason: String,
    round: &str,
    sha: &str,
  ) -> Result<Decision, String> {
    let text = if self.mode == GateMode::Ask {
      if !sha.is_empty() {
        let waiver = self.waivers();
        let _pended = waiver.pend(sha, &self.base, code, SystemTime::now());
      }
      format!("{ASK_LEAD}\n\n{reason}")
    } else {
      reason
    };
    let result = if self.mode == GateMode::Block {
      "deny"
    } else {
      self.mode.name()
    };
    self.record(result, code, round);
    decided(self.mode, text)
  }

  fn waivers(&self) -> Waivers<'_> {
    Waivers {
      roots: &self.state.roots,
      root: Some(&self.root),
      slug: &self.slug,
    }
  }

  fn reviewed(&mut self, branch: &str, file: &Path) -> Result<Decision, String> {
    if !ref_exists(&self.root, &self.base, self.ctx.env) {
      return self.decide(
        "base-missing",
        format!(
          "base branch '{}' not found locally; run `git fetch origin {}:{}`",
          self.base, self.base, self.base
        ),
        "",
        "",
      );
    }
    if branch.is_empty() || branch == "HEAD" {
      return self.decide("detached-head", "detached HEAD — checkout a branch, or push an explicit `HEAD:<branch>` refspec so the review state can be keyed to that branch".to_owned(), "", "");
    }
    if branch == self.base {
      return Ok(Decision::Allow);
    }
    let Some(current) = diff_sha(self.ctx.env, &self.root, &self.base) else {
      self.record("allow", "diff-failed", "");
      return Ok(Decision::Allow);
    };
    if current == EMPTY_BLOB_SHA {
      return self.decide("empty-diff", format!("Refusing to push: diff against {} is empty. Either no commits diverged from base, or the branch was force-reset. Verify intent before pushing.", self.base), "", "empty-diff");
    }
    if self.waivers().matches(&current) {
      self.record("allow", "waived", "");
      return Ok(Decision::Allow);
    }
    self.review_state(file, &current)
  }

  fn review_state(&mut self, file: &Path, current: &str) -> Result<Decision, String> {
    let file_text = file.display().to_string();
    if !std::fs::metadata(file).is_ok_and(|meta| meta.is_file()) {
      return self.decide(
        "no-state",
        no_state_reason(current, &self.base, &file_text),
        "",
        current,
      );
    }
    let doc = read_json(file).unwrap_or(toolu_runtime::json::ordered::Ordered::Null);
    let changed = changed_names(&self.root, &self.base, self.ctx.env);
    if let Some(failure) = state_failure(&doc, &file_text, current, &self.base, &changed) {
      return self.decide(failure.code, failure.reason, &failure.round, current);
    }
    self.record("allow", "pass", &state_round(&doc));
    Ok(Decision::Allow)
  }
}

fn no_state_reason(sha: &str, base: &str, file: &str) -> String {
  format!(
    "Code review required before push (diff SHA {sha}, base {base}).\nRun a code reviewer on `git diff {base}...HEAD` and apply its findings — use {HINT}. Then atomically write {file} (tmp+mv) with schema {{ version: 2, branch, diff_sha, base_branch, reviewed_at, reviewers, findings_count, findings, review_round, reviewed_files }}. `reviewers` must include at least one accepted reviewer (code-review, toolu-review:review, code-review:xhigh, review, or security-review), `findings_count` must be 0, `review_round` starts at 1 for a new `diff_sha` and bumps by 1 only when rewriting at the same `diff_sha`. `reviewed_files` must list every path from `git diff {base}...HEAD --name-only` (sorted, unique) — the actual reviewer file coverage. Retry push."
  )
}

fn evaluate(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  warnings: &mut Vec<String>,
) -> Result<Decision, String> {
  let Some(PushTarget { root, branch }) = push_target(event, ctx) else {
    return Ok(Decision::Allow);
  };
  let config = gate_config(ctx);
  let mode = pre_mode(&config, "pushReview", ctx, true);
  warnings.extend(config.take_warnings());
  if mode == GateMode::Off {
    return Ok(Decision::Allow);
  }
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let slug = branch_slug(&branch);
  let base = ctx
    .env
    .get("PUSH_REVIEW_BASE")
    .map_or_else(|| base_branch(ctx.env, Some(&root), &root), str::to_owned);
  let state = StateCtx::new(roots);
  let mut push = Push {
    ctx,
    mode,
    root,
    slug,
    base,
    state,
  };
  let file = PathBuf::from(push.waivers().dir()).join(format!("{}.json", push.slug));
  let result = push.reviewed(&branch, &file);
  warnings.append(&mut push.state.warnings);
  result
}

impl Gate for PushReview {
  fn name(&self) -> &'static str {
    "push-review"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    evaluate(event, ctx, &mut Vec::new())
  }

  fn run_warning(
    &self,
    event: &NormalizedEvent,
    ctx: &RuleContext<'_>,
    warnings: &mut Vec<String>,
  ) -> Result<Decision, String> {
    evaluate(event, ctx, warnings)
  }
}

#[cfg(test)]
#[path = "tests/push_review_test.rs"]
mod tests;
