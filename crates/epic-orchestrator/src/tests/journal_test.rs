use std::fs;
use std::time::UNIX_EPOCH;

use super::{Record, append, file_for, parse, retain, tail};

#[test]
fn append_is_ordered_and_a_partial_line_is_ignored() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let lock = root.join("journal.lock");
  let journal = root.join("journal");
  let now = UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
  let first = append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "a", "t1", "ok"),
    now,
  )
  .unwrap();
  assert_eq!(first.seq, 1);
  fs::write(file_for(&journal, now), "{\"seq\":1}\n{\"seq\":").unwrap();
  let second = append(
    &journal,
    &lock,
    &Record::new("action", "merge-done", "a", "t1", "ghp_secret"),
    now,
  )
  .unwrap();
  assert_eq!(second.note, "[redacted]");
  let rows = tail(&journal, now).unwrap();
  assert!(
    rows
      .iter()
      .any(|row| row.seq == second.seq && row.name == "merge-done")
  );
  retain(&journal, 0, now).unwrap();
}

#[test]
fn append_waits_for_a_brief_holder() {
  let dir = tempfile::tempdir().unwrap();
  let lock = dir.path().join("journal.lock");
  let journal = dir.path().join("journal");
  let held = crate::lock::Held::acquire(&lock, "journal-busy").expect("hold");
  let worker = std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_millis(80));
    drop(held);
  });
  let now = UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
  let stored = append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "a", "t1", "ok"),
    now,
  )
  .expect("append");
  worker.join().expect("join");
  assert_eq!(stored.seq, 1);
}

#[test]
fn status_journal_sequence_survives_a_day_without_events() {
  let dir = tempfile::tempdir().expect("temp root");
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  let first_day = UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
  let later_day = first_day + std::time::Duration::from_hours(72);
  let first = append(
    &journal,
    &lock,
    &Record::new("transition", "brainstorm", "issue-a", "private", "first"),
    first_day,
  )
  .expect("first record");
  let later = append(
    &journal,
    &lock,
    &Record::new("transition", "execution", "issue-a", "private", "later"),
    later_day,
  )
  .expect("later record");
  assert_eq!(later.seq, first.seq + 1);
}

#[test]
fn each_streams_days_from_a_given_day_in_date_order_without_partial_lines() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  fs::create_dir_all(&journal).unwrap();
  let line = |seq: u64, name: &str| {
    format!("{{\"seq\":{seq},\"at\":\"x\",\"kind\":\"action\",\"name\":\"{name}\"}}\n")
  };
  fs::write(journal.join("2026-10-01.jsonl"), line(1, "old")).unwrap();
  fs::write(
    journal.join("2026-10-03.jsonl"),
    format!("{}{{\"seq\":4", line(3, "late")),
  )
  .unwrap();
  fs::write(journal.join("2026-10-02.jsonl"), line(2, "kept")).unwrap();
  fs::write(journal.join("notes.txt"), "ignored").unwrap();
  let mut names = Vec::new();
  super::each(&journal, "2026-10-02", |row| {
    names.push(row.name);
    Ok(())
  })
  .unwrap();
  assert_eq!(names, ["kept", "late"]);
  let mut none = 0;
  super::each(&dir.path().join("absent"), "", |_row| {
    none += 1;
    Ok(())
  })
  .unwrap();
  assert_eq!(none, 0);
}

#[test]
fn parse_uses_empty_text_for_missing_or_non_string_fields_and_requires_seq() {
  let row = parse(r#"{"seq":7,"kind":"action","name":false,"note":123,"key":{"nested":true}}"#)
    .expect("valid sequence");
  assert_eq!(row.seq, 7);
  assert_eq!(
    (row.kind.as_str(), row.name.as_str(), row.note.as_str()),
    ("action", "", "")
  );
  assert_eq!(
    (row.at.as_str(), row.key.as_str(), row.token.as_str()),
    ("", "", "")
  );
  assert!(parse(r#"{"note":"no sequence"}"#).is_none());
  assert!(parse(r#"{"seq":-1,"note":"negative sequence"}"#).is_none());
}

#[test]
fn each_with_skips_other_lines_and_keeps_escaped_token_candidates() {
  let dir = tempfile::tempdir().expect("temp");
  let journal = dir.path().join("journal");
  fs::create_dir_all(&journal).expect("journal");
  let token = "quote\" and newline\n";
  let line = |seq: u64, field: &str, value: &str| {
    let mut row = serde_json::json!({"seq": seq, "token": "other", "note": "unrelated"});
    row[field] = serde_json::json!(value);
    format!("{row}\n")
  };
  fs::write(journal.join("2026-10-01.jsonl"), line(1, "token", token)).expect("old");
  fs::write(
    journal.join("2026-10-02.jsonl"),
    format!(
      "{}{}{}{{\"seq\":4",
      line(2, "note", token),
      line(3, "token", token),
      line(9, "note", "nothing")
    ),
  )
  .expect("current");
  let mut candidates = Vec::new();
  super::each_with(&journal, "2026-10-02", token, |row| {
    candidates.push((row.seq, row.token == token));
    Ok(())
  })
  .expect("scan");
  assert_eq!(candidates, [(2, false), (3, true)]);
}
