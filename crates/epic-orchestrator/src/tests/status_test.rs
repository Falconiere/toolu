use super::document;
use crate::model::World;

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
