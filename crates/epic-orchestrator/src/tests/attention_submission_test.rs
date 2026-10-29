//! Submission-id admit, dedupe, and restart behaviour for `answer`.

use std::path::Path;

use serde_json::{Value, json};

use super::tests::{answer, engine, prompts, report, rows};
use crate::model::Ack;

fn submitted(text: &str, id: &str) -> Value {
  json!({"op": "answer", "key": "a", "text": text, "by": "web", "submission_id": id})
}

fn named(engine: &crate::server::Engine, name: &str) -> Vec<(String, String, String)> {
  rows(engine)
    .into_iter()
    .filter(|row| row.name == name)
    .map(|row| (row.kind, row.token, row.note))
    .collect()
}

#[test]
fn a_submission_id_admits_before_prompting_and_dedupes_the_same_payload() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  let request = submitted("take B", "s-1");
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 1);
  let admits = named(&engine, "answer-admit");
  assert_eq!(admits.len(), 1, "{admits:?}");
  let (kind, token, note) = &admits[0];
  assert_eq!((kind.as_str(), token.as_str()), ("action", "s-1"));
  let fp = super::fingerprint("a", "take B");
  assert_eq!(fp.len(), 64);
  assert!(
    fp.bytes()
      .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
  );
  assert_eq!(note, &format!("fp={fp} by=web"));
  let delivered = named(&engine, "answer");
  assert!(
    delivered
      .iter()
      .any(|(kind, token, _)| kind == "action" && token == "s-1"),
    "{delivered:?}"
  );
}

#[test]
fn a_fingerprint_is_the_sha256_of_key_nul_text() {
  // `printf 'a\0take B' | shasum -a 256`
  assert_eq!(
    super::fingerprint("a", "take B"),
    "38677b0b0100f2c0b2a12a6e34b32badc064cee9e1f105eac3910f5e965b5518"
  );
  assert_eq!(
    super::fingerprint("", ""),
    "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
  );
  assert_ne!(super::fingerprint("ab", "c"), super::fingerprint("a", "bc"));
}

#[test]
fn a_reused_submission_id_with_other_text_is_refused_without_a_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  assert_eq!(
    answer(&mut engine, &submitted("one", "s-1")),
    json!({"ok": true})
  );
  assert_eq!(
    answer(&mut engine, &submitted("two", "s-1")),
    json!({"error": "submission_id reuse with different payload"})
  );
  assert_eq!(prompts(tmp.path()), [json!({"key": "a", "text": "one"})]);
  assert_eq!(
    answer(&mut engine, &submitted("one", "s-2")),
    json!({"ok": true})
  );
  assert_eq!(prompts(tmp.path()).len(), 2);
}

#[test]
fn an_uncertain_submission_keeps_its_token_and_a_retry_does_not_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "uncertain");
  report(&mut engine, "needs-human");
  let first = answer(&mut engine, &submitted("take B", "s-1"));
  assert_eq!(first["delivered"], "uncertain");
  let retry = answer(&mut engine, &submitted("take B", "s-1"));
  assert_eq!(retry["delivered"], "uncertain");
  assert_eq!(prompts(tmp.path()).len(), 1);
  for (kind, token, _) in named(&engine, "answer-uncertain") {
    assert_eq!(token, "s-1", "{kind}");
  }
  assert_eq!(named(&engine, "answer-uncertain").len(), 2);
}

#[test]
fn a_refused_prompt_lets_the_same_submission_id_try_again() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "failed");
  report(&mut engine, "needs-human");
  let request = submitted("take B", "s-1");
  let refused = json!({"error": "worker prompt not delivered: prompt failed"});
  assert_eq!(answer(&mut engine, &request), refused);
  assert_eq!(answer(&mut engine, &request), refused);
  assert_eq!(prompts(tmp.path()).len(), 2);
  assert_eq!(named(&engine, "answer-refused").len(), 2);
}

#[test]
fn a_reopened_engine_dedupes_from_the_retained_journal_only() {
  let tmp = tempfile::tempdir().expect("temp");
  let request = submitted("take B", "s-1");
  {
    let mut first = engine(tmp.path(), "sent");
    report(&mut first, "needs-human");
    assert_eq!(answer(&mut first, &request), json!({"ok": true}));
  }
  let mut again = engine(tmp.path(), "sent");
  report(&mut again, "needs-human");
  assert_eq!(answer(&mut again, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 1, "the admit row survived");
  std::fs::remove_dir_all(again.paths.journal_dir()).expect("age out");
  assert_eq!(answer(&mut again, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 2);
}

#[test]
fn an_admit_older_than_the_tail_window_does_not_dedupe() {
  use std::time::{Duration, SystemTime, UNIX_EPOCH};

  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  let journal = engine.paths.journal_dir();
  std::fs::create_dir_all(&journal).expect("journal");
  let fp = super::fingerprint("a", "take B");
  let old = UNIX_EPOCH + Duration::from_secs(1_577_836_800); // 2020-01-01
  let line = json!({
    "seq": 1,
    "at": "2020-01-01T00:00:00.000Z",
    "kind": "action",
    "name": "answer-admit",
    "epic": "",
    "key": "a",
    "token": "s-old",
    "note": format!("fp={fp} by=web"),
  });
  std::fs::write(
    crate::journal::file_for(&journal, old),
    format!("{line}\n"),
  )
  .expect("old admit");
  engine.now = SystemTime::now();
  let request = submitted("take B", "s-old");
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 1, "outside the tail window");
}

#[test]
fn a_delivered_submission_dedupes_after_the_agent_is_gone() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  let request = submitted("take B", "s-1");
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  engine.world.issues.get_mut("a").expect("issue").stage = "merged".to_owned();
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 1);
}

#[test]
fn a_no_live_agent_refusal_does_not_poison_a_later_admission() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  let request = submitted("take B", "s-1");
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "no live agent for a"})
  );
  assert_eq!(prompts(tmp.path()), [] as [Value; 0]);
  assert_eq!(
    named(&engine, "answer-admit"),
    [] as [(String, String, String); 0]
  );
  report(&mut engine, "needs-human");
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 1);
}

fn breaking_script(root: &Path) {
  let script = format!(
    "cat >> '{prompts}'\necho >> '{prompts}'\nrm -rf '{journal}'\n: > '{journal}'\nprintf '{{\"outcome\":\"sent\"}}'\n",
    prompts = root.join("prompts").display(),
    journal = root.join("journal").display(),
  );
  std::fs::write(root.join("scripts").join("prompt"), script).expect("script");
}

#[test]
fn a_journal_failure_after_the_prompt_is_uncertain_and_leaves_attention_open() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  breaking_script(tmp.path());
  let reply = answer(&mut engine, &submitted("take B", "s-1"));
  assert_eq!(reply["ok"], true, "{reply}");
  assert_eq!(reply["delivered"], "uncertain", "{reply}");
  assert!(
    reply["note"]
      .as_str()
      .is_some_and(|note| note.contains("journal")),
    "{reply}"
  );
  assert_eq!(prompts(tmp.path()).len(), 1);
  assert_eq!(engine.world.attention[0].state, Ack::Open);
  assert!(engine.world.outbox.is_empty());
}

fn chmod_tree(root: &Path, file_mode: u32, dir_mode: u32) {
  use std::os::unix::fs::PermissionsExt;
  for entry in std::fs::read_dir(root).expect("list") {
    let path = entry.expect("entry").path();
    let mut perms = std::fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(file_mode);
    std::fs::set_permissions(&path, perms).expect("file mode");
  }
  let mut perms = std::fs::metadata(root).expect("meta").permissions();
  perms.set_mode(dir_mode);
  std::fs::set_permissions(root, perms).expect("dir mode");
}

#[test]
fn a_refuse_journal_failure_fails_closed_as_uncertain() {
  use crate::journal::{self, Record};

  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  let request = submitted("take B", "s-1");
  let fp = super::fingerprint("a", "take B");
  let admit = Record::new(
    "action",
    "answer-admit",
    "a",
    "s-1",
    &format!("fp={fp} by=web"),
  );
  journal::append(
    &engine.paths.journal_dir(),
    &engine.paths.journal_lock(),
    &admit,
    engine.now,
  )
  .expect("admit");
  let journal = engine.paths.journal_dir();
  chmod_tree(&journal, 0o444, 0o555);
  let refused = super::refuse_admitted(
    &mut engine,
    &super::Answer {
      key: "a",
      text: "take B",
      by: "web",
      submission: Some("s-1"),
    },
  );
  assert!(refused.is_err(), "{refused:?}");
  chmod_tree(&journal, 0o644, 0o755);
  // Open admit remains: same-id retry stays uncertain (no re-prompt after restart either).
  assert_eq!(answer(&mut engine, &request)["delivered"], "uncertain");
  assert_eq!(prompts(tmp.path()).len(), 0);
}

#[test]
fn a_refused_submission_id_still_rejects_a_different_payload() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "failed");
  report(&mut engine, "needs-human");
  let refused = json!({"error": "worker prompt not delivered: prompt failed"});
  assert_eq!(answer(&mut engine, &submitted("one", "s-1")), refused);
  assert_eq!(
    answer(&mut engine, &submitted("two", "s-1")),
    json!({"error": "submission_id reuse with different payload"})
  );
  assert_eq!(prompts(tmp.path()).len(), 1);
}
