use super::*;
use crate::hosts::HostKind;
use crate::launch::{LaunchOpts, Resolved};

#[test]
fn recovery_refuses_a_different_core_session_before_any_mutation() {
  let graph = crate::support::graph_248();
  let issue = graph.issues.first().unwrap();
  let flags = LaunchOpts::default();
  let host = Resolved {
    kind: HostKind::Claude,
    model: None,
    effort: None,
  };
  let attempt = Attempt::new(&graph, issue, &flags, host, 0);
  let env = toolu_runtime::env::Env::from_pairs([("HC_SESSION", "other")]);
  let paths = crate::paths::Paths::from_env(&env);
  let ctx = Ctx::live(&env, &paths, Err("offline".into()));
  let mut record = Record::new();
  record.insert("hc_session".into(), json!("original"));
  let previous = record.clone();
  assert!(
    attempt
      .hc_recover(&mut record, &ctx)
      .unwrap_err()
      .message
      .contains("HC_SESSION")
  );
  assert_eq!(record, previous);
}
