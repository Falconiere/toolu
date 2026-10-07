use super::{spool_path, with_protocol};

#[test]
fn a_request_keeps_its_fields_and_records_the_protocol() {
  let body = with_protocol(&serde_json::json!({"op": "wait", "max_seconds": 0}), 1);
  assert_eq!(body["op"], "wait");
  assert_eq!(body["protocol"], 1);
  let path = spool_path(std::path::Path::new("/tmp/epic"), "t1");
  assert_eq!(path, std::path::PathBuf::from("/tmp/epic/spool/t1.json"));
}
