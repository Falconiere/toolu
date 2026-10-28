//! The findings cap: secrets win slots over ast-grep, and a drop is always reported.

use std::cmp::Reverse;

use super::{Notice, NoticeKind};
use crate::contract::Finding;

/// Worst first, then path and line, so a cap drops the least important.
fn sort(findings: &mut [Finding]) {
  findings.sort_by(|a, b| {
    (Reverse(a.severity), &a.path, a.line).cmp(&(Reverse(b.severity), &b.path, b.line))
  });
}

/// Secrets then ast-grep findings, each group sorted, at most `max` in all. The
/// notice is `Some` exactly when something was dropped.
pub(super) fn cap(
  mut secrets: Vec<Finding>,
  mut structural: Vec<Finding>,
  max: usize,
) -> (Vec<Finding>, Option<Notice>) {
  sort(&mut secrets);
  sort(&mut structural);
  let total = secrets.len() + structural.len();
  secrets.truncate(max);
  structural.truncate(max - secrets.len());
  secrets.extend(structural);
  let dropped = total - secrets.len();
  let notice = (dropped > 0).then(|| Notice {
    kind: NoticeKind::Overflow,
    text: format!("prepass: capped at {max} findings (dropped {dropped})"),
  });
  (secrets, notice)
}

#[cfg(test)]
#[path = "tests/cap_test.rs"]
mod tests;
