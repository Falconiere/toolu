use toolu_shell::analyze;

use super::{Verdict, verdict};

#[test]
fn allow_override_is_scoped_to_the_matching_simple_command() {
  let analysis = analyze("node -e 1 && cargo test");
  let deny = vec!["node -e".to_owned(), "cargo test".to_owned()];
  let allow = vec!["node -e".to_owned()];
  assert_eq!(
    verdict(&analysis, &allow, &deny),
    Verdict::Deny("cargo test".to_owned())
  );
}

#[test]
fn a_heredoc_body_is_not_a_rule_hit() {
  let analysis = analyze("cat <<'EOF'\nnode -e 1\nEOF");
  assert_eq!(
    verdict(&analysis, &[], &["node -e".to_owned()]),
    Verdict::Allow
  );
}

#[test]
fn unknown_analysis_is_a_guardrail_hit() {
  let analysis = analyze(&"x".repeat(toolu_shell::MAX_SHELL_INPUT + 1));
  assert!(matches!(
    verdict(&analysis, &[], &["node -e".to_owned()]),
    Verdict::Unknown(_)
  ));
}
