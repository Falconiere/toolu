//! Re-anchoring for `validate`: a model finding whose line numbers miss its
//! verbatim quote moves to the one run of changed lines that holds that quote,
//! instead of being dropped and costing its path's coverage.

use std::collections::BTreeMap;

use super::{Anchors, collapse_whitespace};
use crate::contract::Side;
use crate::model::ModelFinding;
use crate::review::mask::Mask;

/// Fewest non-space characters a quote needs to be re-anchored, as the recheck
/// asks of its evidence quotes; shorter ones (`}`, `Ok(())`) sit on many lines.
const MIN_QUOTE_CHARS: usize = 10;

/// `finding` moved to the only run of consecutive changed lines on its path and
/// side whose masked text equals its masked quote, or `None` when the quote is
/// too short or no single run holds it.
pub(super) fn reanchor(
  finding: &ModelFinding,
  anchors: &Anchors<'_>,
  mask: &Mask,
) -> Option<ModelFinding> {
  let quote = mask.apply(&finding.quote);
  let want = collapse_whitespace(&quote);
  if want.chars().filter(|c| !c.is_whitespace()).count() < MIN_QUOTE_CHARS {
    return None;
  }
  let old = finding.side == Side::Old;
  let lines: BTreeMap<u32, &str> = anchors
    .iter()
    .filter(|((path, side, _), _)| *path == finding.path && *side == old)
    .map(|((_, _, line), text)| (*line, *text))
    .collect();
  let span = u32::try_from(quote.lines().count().max(1)).ok()?;
  let mut starts = lines.keys().copied().filter(|start| {
    (*start..start.saturating_add(span))
      .map(|line| lines.get(&line).copied())
      .collect::<Option<Vec<_>>>()
      .is_some_and(|texts| collapse_whitespace(&mask.apply(&texts.join("\n"))) == want)
  });
  let start = starts.next()?;
  if starts.next().is_some() {
    return None;
  }
  let mut moved = finding.clone();
  moved.line = start;
  moved.end_line = start + span - 1;
  Some(moved)
}

#[cfg(test)]
#[path = "tests/reanchor_test.rs"]
mod tests;
