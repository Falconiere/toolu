use serde_json::json;

use super::comment;

#[test]
fn a_pending_intent_recovers_the_original_note_after_a_lost_post_response() {
  let dir = tempfile::tempdir().expect("temp");
  let first = comment(
    dir.path(),
    ("github", "issue", "O/R#2"),
    "original",
    |_| Ok(false),
    |_| Err("response lost".to_owned()),
  );
  assert_eq!(first, Err("response lost".to_owned()));
  let second = comment(
    dir.path(),
    ("github", "issue", "O/R#2"),
    "changed",
    |body| {
      assert_eq!(body, "original");
      Ok(true)
    },
    |_| panic!("comment must not be reposted"),
  );
  assert_eq!(second, Ok(()));
  let receipts = std::fs::read_dir(dir.path().join("close-intents"))
    .expect("receipts")
    .filter_map(Result::ok)
    .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
    .collect::<Vec<_>>();
  assert_eq!(receipts.len(), 1);
  let value: serde_json::Value =
    serde_json::from_slice(&std::fs::read(receipts[0].path()).expect("receipt")).expect("json");
  assert_eq!(value, json!({"note": "original", "delivered": true}));
}
