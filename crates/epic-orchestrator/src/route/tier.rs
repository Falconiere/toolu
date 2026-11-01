//! Complexity tiers: Jev's expected score nudged upward, or a heuristic.

use regex::Regex;

/// The evidence a tier is judged from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Evidence {
  /// The herdr key.
  pub key: String,
  /// The title.
  pub title: String,
  /// Labels.
  pub labels: Vec<String>,
  /// Description excerpt.
  pub excerpt: String,
  /// Open blockers.
  pub open_blockers: usize,
  /// How much work waits on it.
  pub unblocks: u64,
}

/// The score levels Jev rates on.
pub(crate) const LEVELS: [&str; 4] = [
  "Trivial: a docs, config, copy, or one-line fix with no design work.",
  "Standard: a contained change in one module that follows existing patterns and has clear acceptance criteria.",
  "Complex: spans several modules or needs new design, a migration, concurrency, or substantial new tests.",
  "Critical: cross-cutting architecture, security or data-loss risk, or ambiguous requirements that need deep reasoning.",
];

/// The deterministic fallback when Jev is unavailable.
pub(crate) fn heuristic_tier(evidence: &Evidence) -> usize {
  let text = format!("{} {}", evidence.title, evidence.labels.join(" ")).to_lowercase();
  let words = |pattern: &str| Regex::new(pattern).is_ok_and(|re| re.is_match(&text));
  if words(r"\b(security|architecture|breaking|data[- ]loss|migration|rewrite)\b") {
    return 3;
  }
  if words(r"\b(docs?|typo|readme|copy|chore|bump|config)\b") {
    return 0;
  }
  let tasks =
    Regex::new(r"(?m)^\s*[-*]\s*\[[ xX]\]").map_or(0, |re| re.find_iter(&evidence.excerpt).count());
  let long = evidence.excerpt.chars().count() > 1200;
  if long || tasks >= 6 || evidence.unblocks >= 3 {
    2
  } else {
    1
  }
}

/// Jev's expected score, rounded after a small upward bias: under-provisioning
/// a hard issue costs a failed run; over-provisioning an easy one costs tokens.
pub(crate) fn tier_from_score(score: f64) -> usize {
  let rounded = (score + 0.15).round().clamp(0.0, 3.0);
  [1.0, 2.0, 3.0]
    .iter()
    .map(|step| usize::from(rounded >= *step))
    .sum()
}

#[cfg(test)]
#[path = "tests/tier_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/epic_creator_tier_test.rs"]
mod epic_creator_tests;
