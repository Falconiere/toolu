//! Submission IDs outside the two-day journal window.

use serde_json::json;

use super::tests::{answer, engine, prompts, report};

#[test]
fn a_future_dated_admit_does_not_dedupe() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  let id = "future-id";
  let fp = super::fingerprint("a", "take B");
  let future = engine.now + std::time::Duration::from_hours(48);
  let row = json!({"seq": 100, "kind": "action", "name": "answer-admit", "key": "a", "token": id, "note": format!("fp={fp} by=web")});
  std::fs::write(
    crate::journal::file_for(&engine.paths.journal_dir(), future),
    format!("{row}\n"),
  )
  .expect("future admit");
  assert_eq!(
    answer(
      &mut engine,
      &json!({"op": "answer", "key": "a", "text": "take B", "by": "web", "submission_id": id})
    ),
    json!({"ok": true})
  );
  assert_eq!(prompts(tmp.path()).len(), 1);
}
