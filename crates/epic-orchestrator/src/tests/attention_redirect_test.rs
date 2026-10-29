use std::path::Path;

use serde_json::{Value, json};

use super::tests::{answer, engine, prompts, report, rows};
use crate::model::Ack;

#[test]
fn answer_for_an_unknown_issue_calls_no_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "no live agent for a"})
  );
  assert_eq!(prompts(tmp.path()), [] as [Value; 0]);
  assert!(rows(&engine).iter().all(|row| row.name != "answer"));
}

#[test]
fn answer_for_a_merged_issue_calls_no_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "execution");
  engine.world.issues.get_mut("a").expect("issue").stage = "merged".to_owned();
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "no live agent for a"})
  );
  assert_eq!(prompts(tmp.path()), [] as [Value; 0]);
}

#[test]
fn answer_redirects_a_live_worker_with_no_open_item() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "execution");
  let request = json!({"op": "answer", "key": "a", "text": "NEW DIRECTION", "by": "user"});
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 1);
  let sent: Vec<(String, String)> = rows(&engine)
    .into_iter()
    .filter(|row| row.name == "answer")
    .map(|row| (row.key, row.note))
    .collect();
  assert_eq!(sent, [("a".to_owned(), "NEW DIRECTION".to_owned())]);
  let directed: Vec<String> = rows(&engine)
    .into_iter()
    .filter(|row| row.name == "directed")
    .map(|row| row.note)
    .collect();
  assert_eq!(directed, ["by=user"]);
  let direction = engine.world.issues["a"]
    .direction
    .clone()
    .expect("direction");
  assert_eq!(
    (direction.text.as_str(), direction.by.as_str()),
    ("NEW DIRECTION", "user")
  );
}

#[test]
fn a_redirect_not_delivered_records_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "failed");
  report(&mut engine, "execution");
  let request = json!({"op": "answer", "key": "a", "text": "go"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "worker prompt not delivered: prompt failed"})
  );
  assert!(engine.world.issues["a"].direction.is_none());
  assert!(rows(&engine).iter().all(|row| row.name != "answer"));
}

#[test]
fn an_uncertain_answer_replies_ok_journals_it_and_leaves_attention_open() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "uncertain");
  report(&mut engine, "needs-human");
  let request = json!({"op": "answer", "key": "a", "text": "take B", "by": "web"});
  let reply = answer(&mut engine, &request);
  assert_eq!(reply["ok"], true);
  assert_eq!(reply["delivered"], "uncertain");
  assert!(
    reply["note"]
      .as_str()
      .is_some_and(|note| note.contains("prompt uncertain"))
  );
  assert_eq!(prompts(tmp.path()).len(), 1);
  assert!(crate::attention::has_live(&engine.world, "a"));
  let journaled: Vec<(String, String)> = rows(&engine)
    .into_iter()
    .filter(|row| row.name == "answer-uncertain")
    .map(|row| (row.key, row.note))
    .collect();
  assert_eq!(
    journaled,
    [
      ("a".to_owned(), "take B".to_owned()),
      ("a".to_owned(), "by=web".to_owned())
    ]
  );
}

#[test]
fn herdr_timeout_and_stall_errors_are_uncertain_and_others_are_not() {
  let error = |code: &str| {
    let reply = json!({"error": {"code": code, "message": "agent busy"}}).to_string();
    crate::herdr_calls::delivered("agent.prompt", &reply).expect_err("herdr error")
  };
  assert!(super::uncertain(&error("timeout")));
  assert!(super::uncertain(&error("agent_prompt_stalled")));
  assert!(super::uncertain("read: timed out"));
  assert!(!super::uncertain(&error("agent_not_found")));
  assert!(!super::uncertain("prompt failed"));
}

#[test]
fn post_write_socket_failures_are_uncertain_and_pre_send_ones_are_not() {
  for err in [
    "write: Broken pipe (os error 32)",
    "read: connection reset by peer",
    "read: Connection reset by peer (os error 104)",
    "send: broken pipe",
  ] {
    assert!(super::uncertain(err), "{err}");
  }
  for err in [
    "connect: Connection refused (os error 61)",
    "connect: No such file or directory (os error 2)",
    "no live agent for a",
  ] {
    assert!(!super::uncertain(err), "{err}");
  }
}

fn submitted(text: &str, id: &str) -> Value {
  json!({"op": "answer", "key": "a", "text": text, "by": "web", "submission_id": id})
}

/// The journal rows of `name` as (kind, token, note).
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
  // A new id with the same text is an intentional repeat.
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
  // Aged out of retention: the same id is a new admission.
  std::fs::remove_dir_all(again.paths.journal_dir()).expect("age out");
  assert_eq!(answer(&mut again, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 2);
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

/// A prompt script that records the body, then replaces the journal directory
/// with a file so the next journal write fails.
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
