use std::time::Duration;

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_jev_client::Config;
use toolu_runtime::env::Env;

use super::{Scoring, request, scores};
use crate::fixture;
use crate::route::items::{Item, parse};

const PATH: &str = "/v1/systemone";

fn items() -> Vec<Item> {
  parse(&fixture::json("items/review-items.json"), &[]).unwrap()
}

fn config(fixture: &Fixture) -> Config {
  Config {
    endpoint: fixture.url(PATH),
    pause: Duration::from_millis(10),
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
  }
}

fn env(fixture: &Fixture) -> Env {
  Env::from_pairs([
    ("TYPESAFE_API_KEY", "fixture-key".to_owned()),
    ("HTTPS_PROXY", fixture.proxy_url()),
  ])
}

#[test]
fn the_request_keeps_item_order_and_asks_one_score_per_item() {
  let items = items();
  let (state, questions) = request(&items);
  let state: Value = serde_json::from_str(&state).unwrap();
  assert_eq!(state["items"][&items[1].id]["severity"], "high");
  assert_eq!(state["items"][&items[1].id]["kind"], "thread");
  let first = questions.find(&items[0].id).unwrap();
  assert!(first < questions.find(&items[5].id).unwrap());
  let questions: Value = serde_json::from_str(&questions).unwrap();
  let question = &questions[&items[0].id];
  assert_eq!(question["type"], "score");
  assert_eq!(question["criteria"].as_array().unwrap().len(), 4);
  assert!(
    question["instructions"]
      .as_str()
      .unwrap()
      .contains(&format!("items[\"{}\"]", items[0].id))
  );
}

#[test]
fn jev_answers_over_loopback_become_scores() {
  let fixture = Fixture::start().unwrap();
  let answers = fixture::json("jev/fix-tiers.json");
  let body =
    json!({"model": "jev-1", "usage": {"input_tokens": 9, "output_tokens": 3}, "answers": answers});
  fixture
    .route(PATH, Reply::new(200, body.to_string()))
    .unwrap();
  let items = items();
  let scored = scores(&items, Scoring::Ask(&env(&fixture), config(&fixture))).unwrap();
  assert_eq!(scored.note, None);
  assert_eq!(
    scored.answers[&items[0].id],
    json!({"score": 1.92, "confidence": 0.08})
  );
  let sent = fixture.requests().unwrap();
  assert_eq!(sent.len(), 1);
  let sent: Value = serde_json::from_slice(&sent[0].body).unwrap();
  assert_eq!(sent["model"], "jev-latest");
  assert_eq!(
    sent["state"]["items"][&items[2].id]["task"],
    items[2].task.as_str()
  );
}

#[test]
fn a_missing_key_or_a_failing_jev_falls_back_with_a_note() {
  let fixture = Fixture::start().unwrap();
  fixture
    .route(PATH, Reply::new(400, "{\"error\":\"bad\"}"))
    .unwrap();
  let items = items();
  let keyless = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  let missing = scores(&items, Scoring::Ask(&keyless, config(&fixture))).unwrap();
  let expected = "jev unavailable (TYPESAFE_API_KEY not set); heuristic tiers used";
  assert_eq!(missing.note.as_deref(), Some(expected));
  let failed = scores(&items, Scoring::Ask(&env(&fixture), config(&fixture))).unwrap();
  let note = failed.note.unwrap();
  assert!(
    note.starts_with("jev failed: Jev answered HTTP 400"),
    "{note}"
  );
  assert!(note.ends_with("; heuristic tiers used"));
  assert_eq!(failed.answers.len(), 0);
}

#[test]
fn replay_reads_recorded_answers_and_off_says_why() {
  let items = items();
  let replay = scores(
    &items,
    Scoring::Replay(&fixture::path("jev/fix-tiers.json")),
  )
  .unwrap();
  assert_eq!(replay.answers.len(), 6);
  assert_eq!(replay.note, None);
  let off = scores(&items, Scoring::Off).unwrap();
  assert_eq!(
    off.note.as_deref(),
    Some("jev disabled; heuristic tiers used")
  );
  let dir = tempfile::tempdir().unwrap();
  let broken = dir.path().join("answers.json");
  std::fs::write(&broken, "[]").unwrap();
  let failure = scores(&items, Scoring::Replay(&broken)).err().unwrap();
  assert_eq!(failure.code, "usage");
  assert!(
    failure
      .message
      .starts_with("--jev-answers-in is not a JSON object: ")
  );
}
