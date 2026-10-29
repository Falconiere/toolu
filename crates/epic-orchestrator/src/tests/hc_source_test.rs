use super::{Frames, facts};
use crate::source::Fact;
use serde_json::json;

#[test]
fn hc_status_and_finished_map_to_the_exact_terminal() {
  let status = json!({"jsonrpc":"2.0", "method":"event", "params":{"seq": 4, "terminal":"peer/t7", "type":"status", "data":{"status":{"state":"blocked"}}}});
  let rows = facts(&status);
  let Fact::Event(line) = &rows[0] else {
    panic!("status event")
  };
  assert_eq!(line["data"]["pane_id"], "hc:peer/t7");
  assert_eq!(line["data"]["agent_status"], "blocked");
  let finished = json!({"method":"event", "params":{"terminal":"t1", "type":"finished", "data":{"exit_code":0}}});
  let Fact::Event(line) = &facts(&finished)[0] else {
    panic!("finished event")
  };
  assert_eq!(line["data"]["agent_status"], "done");
}

#[test]
fn chunks_reassemble_frames_and_bound_oversized_lines() {
  let (tx, rx) = std::sync::mpsc::channel();
  let mut frames = Frames::default();
  frames.push(br#"{"channel":null,"seq":0,"term"#, &tx, None);
  frames.push(b"inals\":[]}\n", &tx, None);
  assert!(matches!(
    rx.try_recv(),
    Ok(crate::socket_loop::Msg::Herdr(Fact::Snapshot(_)))
  ));
  frames.push(&vec![b'x'; super::FRAME_LIMIT + 1], &tx, None);
  frames.push(b"\n", &tx, None);
  assert!(matches!(
    rx.try_recv(),
    Ok(crate::socket_loop::Msg::Herdr(Fact::HcOutage))
  ));
  assert!(frames.retry);
}

#[test]
fn legacy_engines_do_not_start_an_unselected_hc_core() {
  let mut world = crate::model::World::new(0);
  let env = toolu_runtime::env::Env::from_pairs([("TOOLU_EPIC_MULTIPLEXER", "tmux")]);
  assert!(!super::needed(&env, &std::sync::Mutex::new(world.clone())));
  let mut issue = crate::model::Issue::blank("a", "epic", "/tmp/state", 0);
  issue.agent = "hc:t7".into();
  world.issues.insert("a".into(), issue);
  assert!(super::needed(&env, &std::sync::Mutex::new(world)));
}

#[test]
fn a_missed_notification_reconciles_its_embedded_snapshot() {
  let frame = json!({"method":"missed","params":{"seq":9,"terminals":[
    {"address":"loop/t7","state":"live","labels":{"toolu.owner":"run"},"status":{"state":"blocked"}}
  ]}});
  let facts = super::facts(&frame);
  let Fact::Snapshot(snapshot) = &facts[0] else {
    panic!("missed snapshot")
  };
  assert_eq!(snapshot["agents"][0]["name"], "hc:loop/t7");
  assert_eq!(snapshot["agents"][0]["agent_status"], "blocked");
}
