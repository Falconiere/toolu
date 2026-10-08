use serde_json::{Map, Value};
use toolu_protocol::host::Host;

use super::{delivery_flow_notice, gate_preset_notice};

#[test]
fn notices_show_once_and_a_pinned_preset_skips_the_gate_text() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let first = gate_preset_notice(root, &Map::new()).unwrap();
  assert!(first.contains("balanced"));
  assert!(root.join("toolu").join(".gate-preset-notice-v6").is_file());
  assert_eq!(gate_preset_notice(root, &Map::new()), None);
  let mut gates = Map::new();
  gates.insert("preset".to_owned(), Value::String("strict".to_owned()));
  let mut pinned = Map::new();
  pinned.insert("gates".to_owned(), Value::Object(gates));
  let other = dir.path().join("other");
  assert_eq!(gate_preset_notice(&other, &pinned), None);
  assert!(!other.join("toolu").join(".gate-preset-notice-v6").exists());
  let delivery = delivery_flow_notice(root, Host::Claude).unwrap();
  assert!(delivery.contains("/plugin install delivery-flow@toolu"));
  assert_eq!(delivery_flow_notice(root, Host::Claude), None);
}
