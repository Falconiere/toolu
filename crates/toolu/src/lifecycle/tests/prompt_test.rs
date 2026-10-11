use super::{HintOptions, PromptGate, mentions_gate_topic, prompt_gate, prompt_hints};

const USE_AST: &str = "Structural pattern: use `ast-grep run --pattern` (not Grep).";
const NO_AST: &str =
  "WARN: ast-grep not installed — install via brew/cargo for structural matching.";

fn hints(lower: &str, ast_grep: bool, research: bool) -> Vec<String> {
  prompt_hints(lower, &HintOptions { ast_grep, research })
}

#[test]
fn trivial_replies_and_slash_commands_skip_and_a_bare_verb_blocks() {
  assert_eq!(prompt_gate("y"), PromptGate::Skip);
  assert_eq!(prompt_gate("thanks."), PromptGate::Skip);
  assert_eq!(prompt_gate("/review"), PromptGate::Skip);
  assert_eq!(prompt_gate("fix"), PromptGate::Block);
  assert_eq!(prompt_gate("fix the parser"), PromptGate::Hint);
}

#[test]
fn a_shape_prompt_names_ast_grep_only_when_it_is_usable() {
  let with = hints("list all methods on the trait", true, false);
  let without = hints("find every function that returns a result", false, false);
  assert_eq!(with, vec![USE_AST]);
  assert_eq!(without, vec![NO_AST]);
  assert!(!mentions_gate_topic("list all methods on the trait"));
}

#[test]
fn rename_does_not_match_inside_remove_and_research_is_optional() {
  assert_eq!(
    hints("rename the helper and move it", false, false),
    vec!["Rename: find all refs (ast-grep + Grep on configs) before rewriting."]
  );
  assert_eq!(
    hints("remove the helper", false, false),
    vec!["Verify no deps before removing."]
  );
  let research = hints("what is the latest bun release", false, true);
  assert!(research.iter().any(|hint| hint.contains("research-agent")));
  assert_eq!(
    hints("what is the latest bun release", false, false),
    Vec::<String>::new()
  );
}

#[test]
fn a_new_feature_asks_for_brainstorm_and_a_fix_is_a_gate_topic() {
  let hints = hints("implement a new feature for the dashboard", false, false);
  assert!(hints.iter().any(|hint| hint.contains("brainstorm")));
  assert!(mentions_gate_topic("fix the parser"));
}

#[test]
fn optional_bash_forms_keep_their_boundaries() {
  assert_eq!(prompt_gate("thanks!"), PromptGate::Skip);
  assert_eq!(prompt_gate("thanks!!"), PromptGate::Hint);
  assert_eq!(prompt_gate("fix\t"), PromptGate::Block);
  assert_eq!(prompt_gate("fix."), PromptGate::Hint);
  let trade = hints("weigh the trade-offs of this approach", false, false);
  assert!(trade.iter().any(|hint| hint.contains("brainstorm")));
  let redesign = hints("redesign stays a word of its own", false, false);
  assert!(redesign.iter().any(|hint| hint.contains("brainstorm")));
  let research = hints("look up the best practices", false, true);
  assert!(research.iter().any(|hint| hint.contains("research-agent")));
  assert!(!mentions_gate_topic("typescript"));
}
