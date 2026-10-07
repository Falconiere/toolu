//! The user-prompt-submit ladder (`prompt-hints.ts`). Patterns are the bash
//! `=~` forms: they anchor where the script anchored, and `WB`/`WE` keep
//! `move` out of `remove`.

use regex::Regex;

const STRUCTURAL_HINT: &str = "Structural pattern: use `ast-grep run --pattern` (not Grep).";
const MISSING_AST_GREP: &str =
  "WARN: ast-grep not installed — install via brew/cargo for structural matching.";
const SCALE_HINT: &str = "Possibly large task — if it splits into genuinely independent units, consider decomposing it; if it is really one thread of work, just do it. The orchestrator skill has the test for which.";
const BRAINSTORM_HINT: &str = "Scope may be unresolved — consider the `brainstorm` skill for material design choices; skip it when the request is already bounded or mechanical.";
const RESEARCH_HINT: &str = "External research — delegate to the research-agent subagent (uses native web search and fetch) to keep main context lean.";

const TRIVIAL: &str = r"^(y|n|yes|no|ok|sure|thanks|thank you|go ahead|looks good|lgtm|correct|exactly|right|done|nah|nope|yep|yup|continue)[.!?]?$";
const VAGUE: &str = r"^(fix|help|debug|check|look|see|run|do|try)[ \t\n\v\f\r]*$";

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
  if is_match(TRIVIAL, lower) || lower.starts_with('/') {
    PromptGate::Skip
  } else if is_match(VAGUE, lower) {
    PromptGate::Block
  } else {
    PromptGate::Hint
  }
}

/// A failing gate is worth mentioning unless the prompt is already about fixing.
pub(crate) fn mentions_gate_topic(lower: &str) -> bool {
  is_match(
    &words("fix|resolve|error|warning|test|lint|check|type"),
    lower,
  )
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
  let structural = words(
    "pattern|struct|trait|interface|all functions|all methods|every function|every method|syntax|code structure|signature|return type|where clause|lifetime|closure|macro|decorator|annotation",
  );
  if is_match(&structural, lower) {
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
  const LINES: &[(&str, &str)] = &[
    (
      "rename|move|extract|split",
      "Rename: find all refs (ast-grep + Grep on configs) before rewriting.",
    ),
    (
      "test|spec|coverage",
      "Tests: real-world data only, NO mocks.",
    ),
    (
      "fix|debug|error|bug|issue",
      "Fix in code. Never suppress with disable comments.",
    ),
    ("delete|remove|clean up", "Verify no deps before removing."),
    (
      "review|audit",
      "Review: forbidden syntax, quality gates, test coverage.",
    ),
  ];
  LINES.iter().find_map(|(alternation, text)| {
    is_match(&words(alternation), lower).then(|| (*text).to_owned())
  })
}

fn scale_hint(lower: &str) -> Option<String> {
  is_match(&words("migrate|codebase-wide|throughout|end-to-end"), lower)
    .then(|| SCALE_HINT.to_owned())
}

fn brainstorm_hint(lower: &str) -> Option<String> {
  let designed = words(
    r"brainstorms?|designs?|scopes?|approach(es)?|architectures?|trade-?offs?|redesigns?|overhauls?",
  );
  let new_thing = r"(^|[^a-z])new[ \t\n\v\f\r]+(feature|workflow|system)([^a-z]|$)";
  (is_match(&designed, lower) || is_match(new_thing, lower)).then(|| BRAINSTORM_HINT.to_owned())
}

fn research_hint(lower: &str, enabled: bool) -> Option<String> {
  let pattern = words(
    r"latest|docs for|library docs|api reference|api docs|changelog|release notes|best practices?|look up|search the web|web search|how to use",
  );
  (enabled && is_match(&pattern, lower)).then(|| RESEARCH_HINT.to_owned())
}

fn words(alternation: &str) -> String {
  format!("(^|[^a-z])({alternation})([^a-z]|$)")
}

fn is_match(pattern: &str, text: &str) -> bool {
  Regex::new(pattern).is_ok_and(|re| re.is_match(text))
}

#[cfg(test)]
#[path = "tests/prompt_test.rs"]
mod tests;
