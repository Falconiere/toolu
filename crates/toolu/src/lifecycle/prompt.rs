//! The user-prompt-submit ladder (`prompt-hints.ts`). Patterns are the bash
//! `=~` forms: they anchor where the script anchored, and a non-letter keeps
//! `move` out of `remove`. The terms are a closed list, so this module does
//! not link the `regex` crate.

const STRUCTURAL_HINT: &str = "Structural pattern: use `ast-grep run --pattern` (not Grep).";
const MISSING_AST_GREP: &str =
  "WARN: ast-grep not installed — install via brew/cargo for structural matching.";
const SCALE_HINT: &str = "Possibly large task — if it splits into genuinely independent units, consider decomposing it; if it is really one thread of work, just do it. The orchestrator skill has the test for which.";
const BRAINSTORM_HINT: &str = "Scope may be unresolved — consider the `brainstorm` skill for material design choices; skip it when the request is already bounded or mechanical.";
const RESEARCH_HINT: &str = "External research — delegate to the research-agent subagent (uses native web search and fetch) to keep main context lean.";

const TRIVIAL: &[&str] = &[
  "y",
  "n",
  "yes",
  "no",
  "ok",
  "sure",
  "thanks",
  "thank you",
  "go ahead",
  "looks good",
  "lgtm",
  "correct",
  "exactly",
  "right",
  "done",
  "nah",
  "nope",
  "yep",
  "yup",
  "continue",
];
const VAGUE: &[&str] = &[
  "fix", "help", "debug", "check", "look", "see", "run", "do", "try",
];
const GATE: &[&str] = &[
  "fix", "resolve", "error", "warning", "test", "lint", "check", "type",
];
const STRUCTURAL: &[&str] = &[
  "pattern",
  "struct",
  "trait",
  "interface",
  "all functions",
  "all methods",
  "every function",
  "every method",
  "syntax",
  "code structure",
  "signature",
  "return type",
  "where clause",
  "lifetime",
  "closure",
  "macro",
  "decorator",
  "annotation",
];
const RENAME: &[&str] = &["rename", "move", "extract", "split"];
const TESTS: &[&str] = &["test", "spec", "coverage"];
const FIX: &[&str] = &["fix", "debug", "error", "bug", "issue"];
const DELETE: &[&str] = &["delete", "remove", "clean up"];
const REVIEW: &[&str] = &["review", "audit"];
const SCALE: &[&str] = &["migrate", "codebase-wide", "throughout", "end-to-end"];
const DESIGNED: &[&str] = &[
  "brainstorm",
  "brainstorms",
  "design",
  "designs",
  "scope",
  "scopes",
  "approach",
  "approaches",
  "architecture",
  "architectures",
  "tradeoff",
  "tradeoffs",
  "trade-off",
  "trade-offs",
  "redesign",
  "redesigns",
  "overhaul",
  "overhauls",
];
const NEW_TAILS: &[&str] = &["feature", "workflow", "system"];
const RESEARCH: &[&str] = &[
  "latest",
  "docs for",
  "library docs",
  "api reference",
  "api docs",
  "changelog",
  "release notes",
  "best practice",
  "best practices",
  "look up",
  "search the web",
  "web search",
  "how to use",
];

/// What a lowered prompt should receive.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PromptGate {
  /// Trivial reply or a slash command.
  Skip,
  /// A one-word verb with no object.
  Block,
  /// Hints may apply.
  Hint,
}

/// Whether ast-grep is usable and whether the research nudge is enabled.
pub(crate) struct HintOptions {
  /// `sg` or `ast-grep` is on `PATH` and the skill is enabled.
  pub(crate) ast_grep: bool,
  /// `agents.research-agent` is enabled.
  pub(crate) research: bool,
}

/// Trivial replies and slash commands get nothing; a one-word verb is blocked.
pub(crate) fn prompt_gate(lower: &str) -> PromptGate {
  if is_trivial(lower) || lower.starts_with('/') {
    PromptGate::Skip
  } else if is_vague(lower) {
    PromptGate::Block
  } else {
    PromptGate::Hint
  }
}

/// A failing gate is worth mentioning unless the prompt is already about fixing.
pub(crate) fn mentions_gate_topic(lower: &str) -> bool {
  contains_any(lower, GATE)
}

/// The intent hint, then the independent nudges, in the bash order.
pub(crate) fn prompt_hints(lower: &str, options: &HintOptions) -> Vec<String> {
  [
    intent_hint(lower, options.ast_grep),
    scale_hint(lower),
    brainstorm_hint(lower),
    research_hint(lower, options.research),
  ]
  .into_iter()
  .flatten()
  .collect()
}

fn intent_hint(lower: &str, ast_grep: bool) -> Option<String> {
  if contains_any(lower, STRUCTURAL) {
    let text = if ast_grep {
      STRUCTURAL_HINT
    } else {
      MISSING_AST_GREP
    };
    return Some(text.to_owned());
  }
  intent_line(lower)
}

fn intent_line(lower: &str) -> Option<String> {
  const LINES: &[(&[&str], &str)] = &[
    (
      RENAME,
      "Rename: find all refs (ast-grep + Grep on configs) before rewriting.",
    ),
    (TESTS, "Tests: real-world data only, NO mocks."),
    (FIX, "Fix in code. Never suppress with disable comments."),
    (DELETE, "Verify no deps before removing."),
    (
      REVIEW,
      "Review: forbidden syntax, quality gates, test coverage.",
    ),
  ];
  LINES
    .iter()
    .find_map(|(terms, text)| contains_any(lower, terms).then(|| (*text).to_owned()))
}

fn scale_hint(lower: &str) -> Option<String> {
  contains_any(lower, SCALE).then(|| SCALE_HINT.to_owned())
}

fn brainstorm_hint(lower: &str) -> Option<String> {
  (contains_any(lower, DESIGNED) || new_thing(lower)).then(|| BRAINSTORM_HINT.to_owned())
}

fn research_hint(lower: &str, enabled: bool) -> Option<String> {
  (enabled && contains_any(lower, RESEARCH)).then(|| RESEARCH_HINT.to_owned())
}

fn is_trivial(lower: &str) -> bool {
  TRIVIAL.contains(&without_final_punct(lower))
}

fn without_final_punct(lower: &str) -> &str {
  match lower.as_bytes().last().copied() {
    Some(b'.' | b'!' | b'?') => {
      let Some(end) = lower.len().checked_sub(1) else {
        return lower;
      };
      lower.get(..end).unwrap_or(lower)
    }
    _ => lower,
  }
}

fn is_vague(lower: &str) -> bool {
  VAGUE.contains(&lower.trim_end_matches(is_prompt_space))
}

fn new_thing(text: &str) -> bool {
  text
    .match_indices("new")
    .any(|(at, word)| !letter_before(text, at) && spaced_tail(text, at.saturating_add(word.len())))
}

fn spaced_tail(text: &str, at: usize) -> bool {
  let Some(rest) = text.get(at..) else {
    return false;
  };
  let trimmed = rest.trim_start_matches(is_prompt_space);
  if trimmed.len() == rest.len() {
    return false;
  }
  NEW_TAILS.iter().any(|tail| starts_bounded(trimmed, tail))
}

fn is_prompt_space(ch: char) -> bool {
  matches!(ch, ' ' | '\t' | '\n' | '\u{000b}' | '\u{000c}' | '\r')
}

fn contains_any(text: &str, terms: &[&str]) -> bool {
  terms.iter().any(|term| contains_term(text, term))
}

fn contains_term(text: &str, term: &str) -> bool {
  text
    .match_indices(term)
    .any(|(at, _)| !letter_before(text, at) && !letter_after(text, at.saturating_add(term.len())))
}

fn starts_bounded(text: &str, term: &str) -> bool {
  text.starts_with(term) && !letter_after(text, term.len())
}

fn letter_before(text: &str, at: usize) -> bool {
  text
    .get(..at)
    .and_then(|prefix| prefix.chars().next_back())
    .is_some_and(|ch| ch.is_ascii_lowercase())
}

fn letter_after(text: &str, at: usize) -> bool {
  text
    .get(at..)
    .and_then(|rest| rest.chars().next())
    .is_some_and(|ch| ch.is_ascii_lowercase())
}

#[cfg(test)]
#[path = "tests/prompt_test.rs"]
mod tests;
