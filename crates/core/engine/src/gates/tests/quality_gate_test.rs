use toolu_runtime::json::ordered::Ordered;
use toolu_shell::analyze;
use toolu_state::gate_file::GateRead;

use super::{failing_doc, field, may_ship};

#[test]
fn legacy_failure_is_read_but_malformed_state_is_not() {
  let legacy = Ordered::parse("{\"status\":\"failing\",\"reason\":\"lint\"}").expect("json");
  assert!(
    failing_doc(GateRead::Unrecognized {
      reason: "legacy".to_owned(),
      value: legacy,
    })
    .is_some()
  );
  assert!(failing_doc(GateRead::Malformed("bad JSON".to_owned())).is_none());
}

#[test]
fn jq_style_fields_keep_objects_and_fallbacks() {
  let doc =
    Ordered::parse("{\"reason\":{\"tool\":\"oxlint\"},\"violations\":false}").expect("json");
  assert_eq!(field(&doc, "reason", "x"), "{\n  \"tool\": \"oxlint\"\n}");
  assert_eq!(field(&doc, "violations", "fallback"), "fallback");
}

#[test]
fn only_git_commit_or_push_may_ship_when_analysis_is_known() {
  assert!(may_ship(&analyze("sudo git commit -m 'feat: x'")));
  assert!(may_ship(&analyze("timeout 120 git push")));
  assert!(!may_ship(&analyze("echo git commit later")));
}
