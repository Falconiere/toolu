use std::sync::mpsc;

use serde_json::json;

use super::reply_release;
use crate::server::{Engine, Fault};

#[test]
fn release_answers_ok_or_why_not() {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let (tx, rx) = mpsc::channel();
  reply_release(&mut engine, &json!({"op": "release", "key": "a"}), &tx).expect("not held");
  assert_eq!(rx.recv().expect("reply"), json!({"error": "a is not held"}));
  engine.world.issues.get_mut("a").expect("a").hold =
    Some(json!({"kind": "coupling", "with": "b"}));
  reply_release(&mut engine, &json!({"op": "release", "key": "a"}), &tx).expect("released");
  assert_eq!(
    rx.recv().expect("reply"),
    json!({"ok": true, "key": "a", "released": true})
  );
  let record = crate::disk::read_value(
    &std::path::Path::new(&engine.world.issues["a"].state_dir).join("issues/a.json"),
  )
  .expect("record");
  assert_eq!(
    (record.get("hold"), record["coupling_released"].clone()),
    (None, json!(true))
  );
}
