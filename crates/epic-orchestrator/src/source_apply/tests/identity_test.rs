use serde_json::json;

#[test]
fn hc_agents_match_exact_address_not_a_shared_cwd() {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let engine = crate::server::Engine::open(fix.engine_paths, None, crate::server::Fault::None)
    .expect("engine");
  let mut issue = engine.world.issues["a"].clone();
  issue.agent = "hc:peer/t1".into();
  issue.worktree = Some("/tree".into());
  assert!(super::same_agent(&issue, &json!({"name":"hc:peer/t1"})));
  assert!(!super::same_agent(
    &issue,
    &json!({"name":"hc:peer/t2", "cwd":"/tree"})
  ));
}
