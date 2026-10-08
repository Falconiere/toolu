use super::document;
use crate::logic::ensure_issue;
use crate::model::{Attention, World};

#[test]
fn an_empty_engine_is_down() {
  let world = World::new(0);
  let doc = document(&world, false, None);
  assert_eq!(doc["engine"], "down");
  assert_eq!(doc["paused"], false);
  assert_eq!(doc["epics"], serde_json::json!([]));
  assert_eq!(doc["issues"], serde_json::json!([]));
  assert_eq!(doc["attention"], serde_json::json!([]));
}

#[test]
fn one_epic_keeps_its_issues_and_open_attention() {
  let mut world = World::new(1);
  world.paused.insert("one".to_owned());
  {
    let issue = ensure_issue(&mut world, "a", "one", "/epic");
    issue.phase = "execution".to_owned();
    issue.stage = "running".to_owned();
  }
  ensure_issue(&mut world, "b", "falconiere-toolu-402", "/other");
  world.attention.push(item("a", "one", false));
  world
    .attention
    .push(item("b", "falconiere-toolu-402", true));
  let doc = document(&world, true, Some("one"));
  assert_eq!(doc["engine"], "running");
  assert_eq!(doc["paused"], true);
  assert_eq!(doc["issues"].as_array().expect("issues").len(), 1);
  assert_eq!(doc["epics"].as_array().expect("epics").len(), 1);
  assert_eq!(doc["attention"].as_array().expect("attention").len(), 1);
  let suffix = document(&world, true, Some("402"));
  assert_eq!(suffix["issues"][0]["key"], "b");
}

fn item(key: &str, epic: &str, delivered: bool) -> Attention {
  Attention {
    seq: 1,
    kind: "stall".to_owned(),
    key: key.to_owned(),
    epic: epic.to_owned(),
    note: "n".to_owned(),
    delivered,
  }
}
