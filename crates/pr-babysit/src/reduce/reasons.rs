//! The tick's reasons and decision: escalations first, then signals, and the
//! success rule.

use serde_json::{Value, json};

/// What the reasons are computed from.
pub(crate) struct Facts<'a> {
  /// `pr.state`: `OPEN`, `CLOSED` or `MERGED`.
  pub(crate) pr_state: &'a str,
  /// `pr.mergeable`.
  pub(crate) mergeable: &'a str,
  /// `pass`, `pending` or `fail`.
  pub(crate) ci: &'a str,
  /// `{name, status, url}` per check.
  pub(crate) checks: &'a [Value],
  /// The bot comment's state, `absent` when there is none.
  pub(crate) bot_state: &'a str,
  /// `approved`, `changes` or `none`.
  pub(crate) bot_verdict: &'a str,
  /// Findings in the verdict.
  pub(crate) findings: usize,
  /// The verdict cannot be read.
  pub(crate) degraded: Option<&'static str>,
  /// The bot comment's URL.
  pub(crate) bot_url: Option<&'a str>,
  /// The head SHA.
  pub(crate) head: &'a str,
  /// Recorded fix attempts.
  pub(crate) fix_attempts: u64,
  /// Finding keys recurring from the last round.
  pub(crate) recurring: usize,
  /// The last round rejected a finding.
  pub(crate) had_rejection: bool,
  /// Consecutive rounds with recurring keys.
  pub(crate) streak: u64,
  /// A provider error on the same head, on a new run.
  pub(crate) provider_repeated: bool,
  /// The audit count.
  pub(crate) unresolved: usize,
  /// Actionable threads.
  pub(crate) actionable: usize,
  /// Replied but unresolved threads.
  pub(crate) stale: usize,
  /// `fixer_running` detail when a fixer is running or blocked.
  pub(crate) fixer: Option<String>,
  /// The comparison fields changed since the last tick.
  pub(crate) changed: bool,
}

fn reason(code: &str, detail: impl Into<String>) -> Value {
  json!({"code": code, "detail": detail.into()})
}

impl Facts<'_> {
  /// The review verdict is approved with zero findings.
  pub(crate) fn approved(&self) -> bool {
    self.bot_state == "complete" && self.bot_verdict == "approved" && self.findings == 0
  }

  /// Escalation reasons, in the shipped order.
  pub(crate) fn escalations(&self) -> Vec<Value> {
    let mut out = Vec::new();
    match self.pr_state {
      "MERGED" => out.push(reason("pr_merged", "PR is merged")),
      "CLOSED" => out.push(reason("pr_closed", "PR is closed")),
      _ => {}
    }
    if self.mergeable == "CONFLICTING" {
      out.push(reason("merge_conflict", "mergeable is CONFLICTING"));
    }
    if self.fix_attempts >= 5 {
      let detail = format!("{} fix attempts recorded", self.fix_attempts);
      out.push(reason("fix_attempts_exhausted", detail));
    }
    if self.recurring > 0 && self.had_rejection {
      let detail = format!(
        "{} finding key(s) recurred after a Won't-fix round",
        self.recurring
      );
      out.push(reason("recurrence_after_rejection", detail));
    }
    if self.recurring > 0 && self.streak >= 2 {
      let detail = format!(
        "finding keys recurred on {} consecutive rounds",
        self.streak
      );
      out.push(reason("recurrence_streak", detail));
    }
    if self.provider_repeated {
      let short: String = self.head.chars().take(8).collect();
      let detail = format!("review provider error twice on head {short}");
      out.push(reason("provider_error_repeated", detail));
    }
    out
  }

  fn named(&self, status: &str) -> String {
    let names: Vec<&str> = self
      .checks
      .iter()
      .filter(|check| check.get("status").and_then(Value::as_str) == Some(status))
      .filter_map(|check| check.get("name").and_then(Value::as_str))
      .collect();
    names.join(", ")
  }

  fn ci_signal(&self) -> Value {
    match self.ci {
      "pass" => reason("ci_pass", format!("{} check(s) passed", self.checks.len())),
      "fail" => reason("ci_failed", self.named("fail")),
      _ if self.checks.is_empty() => reason("ci_pending", "no checks reported yet"),
      _ => reason("ci_pending", self.named("pending")),
    }
  }

  fn review_signal(&self) -> Option<Value> {
    match self.bot_state {
      "in_progress" => Some(reason("review_in_progress", "review bot still running")),
      "provider_error" if !self.provider_repeated => Some(reason(
        "provider_error",
        "review bot reported a provider error; rerun the review job once",
      )),
      "complete" if self.approved() => Some(reason(
        "review_approved",
        "bot verdict approved with zero findings",
      )),
      "complete" => Some(reason(
        "review_changes",
        format!(
          "bot verdict {} with {} finding(s)",
          self.bot_verdict, self.findings
        ),
      )),
      _ => self
        .degraded
        .map(|code| reason(code, "bot verdict cannot be read")),
    }
  }

  /// Signal reasons, in the shipped order.
  pub(crate) fn signals(&self) -> Vec<Value> {
    let mut out = vec![self.ci_signal()];
    if self.unresolved == 0 {
      out.push(reason("threads_clear", "no unresolved review threads"));
    }
    if self.actionable > 0 {
      let detail = format!("{} actionable thread(s)", self.actionable);
      out.push(reason("threads_unresolved", detail));
    }
    if self.stale > 0 {
      let detail = format!("{} replied-but-unresolved thread(s)", self.stale);
      out.push(reason("threads_stale_unresolved", detail));
    }
    out.extend(self.review_signal());
    if self.degraded.is_some() {
      let url = self.bot_url.unwrap_or("no bot comment");
      out.push(reason(
        "manual_verify",
        format!("verify review findings manually: {url}"),
      ));
    }
    if self.mergeable == "UNKNOWN" && self.pr_state == "OPEN" {
      let detail = "GitHub has not computed mergeability yet";
      out.push(reason("mergeable_unknown", detail));
    }
    if let Some(detail) = &self.fixer {
      out.push(reason("fixer_running", detail.clone()));
    }
    if !self.changed {
      out.push(reason("unchanged", "nothing changed since the last tick"));
    }
    out
  }

  /// `escalate`, `success` or `keep_going`.
  pub(crate) fn decision(&self, escalated: bool) -> &'static str {
    let ready = self.pr_state == "OPEN"
      && self.ci == "pass"
      && self.unresolved == 0
      && self.mergeable != "UNKNOWN"
      && self.fixer.is_none()
      && (self.approved() || self.degraded.is_some());
    if escalated {
      "escalate"
    } else if ready {
      "success"
    } else {
      "keep_going"
    }
  }
}

#[cfg(test)]
#[path = "tests/reasons_test.rs"]
mod tests;
