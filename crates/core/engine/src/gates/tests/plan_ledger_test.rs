use super::*;

#[test]
fn stale_green_step_is_named_with_its_title() {
  let ledger = Ordered::parse("{\"steps\":[{\"id\":\"lint\",\"title\":\"Run lint\",\"status\":\"green\",\"diff_sha\":\"old\"}]}").expect("json");
  assert_eq!(blockers(&ledger, "new"), "lint: stale — Run lint");
}

#[test]
fn verified_sha_must_match_the_whole_diff() {
  let ledger = Ordered::parse("{\"steps\":[{\"id\":\"lint\",\"title\":\"Run lint\",\"status\":\"green\",\"diff_sha\":\"new\"}],\"verified_sha\":\"old\"}").expect("json");
  let result = steps_decision(GateMode::Block, &ledger, "new", "plan.md").expect("decision");
  assert!(matches!(result, Decision::Deny { .. }));
}
