use serde_json::json;
use toolu_runtime::env::Env;

use super::{Fact, apply};
use crate::server::{Engine, Fault};
use crate::source_fix::fixture;

#[test]
fn a_closed_pane_on_an_idle_issue_does_not_launch() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let issue = engine.world.issues.get_mut("a").expect("issue");
  issue.stage = "idle".to_owned();
  issue.pane = Some("w1:p1".to_owned());
  let line = json!({"event":"pane_closed","data":{"pane_id":"w1:p1"}});
  let env = Env::from_pairs([("HOME", "/tmp")]);
  apply(&mut engine, &env, Fact::Event(line)).expect("apply");
  assert_eq!(engine.world.issues["a"].launches, 0);
}
