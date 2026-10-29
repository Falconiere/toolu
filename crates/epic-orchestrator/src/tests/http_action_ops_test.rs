//! Allowed actions on a real engine: the same replies as the control socket,
//! journaled `by=web`, an admitted request that misses the deadline answered
//! `200 uncertain`, and an engine that never took it answered `503`.

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::tests::{TOKEN, allowed, journal, live, post, raw, token_on_disk};
use crate::PROTOCOL;
use crate::client::exchange_retry;

fn body(response: &str) -> Value {
  let (_head, body) = response.split_once("\r\n\r\n").expect("body");
  serde_json::from_str(body).expect("json body")
}

/// The next SSE `journal` event whose data holds `needle`, within 2 s.
fn streamed(addr: std::net::SocketAddr, needle: &str) -> String {
  let mut stream = TcpStream::connect(addr).expect("connect");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("timeout");
  let request = format!("GET /api/v1/events/stream?since=0 HTTP/1.1\r\nHost: {addr}\r\n\r\n");
  stream
    .write_all(request.as_bytes())
    .expect("stream request");
  let mut reader = BufReader::new(stream);
  let start = Instant::now();
  while start.elapsed() < Duration::from_secs(2) {
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
      break;
    }
    if line.starts_with("data: ") && line.contains(needle) {
      return line;
    }
  }
  String::new()
}

#[test]
fn a_web_pause_pauses_every_epic_streams_its_event_and_names_the_web() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), crate::http_action::DEADLINE);
  let (code, response) = post(live.addr, "pause", &allowed(live.addr), "{}");
  assert_eq!(
    (code, body(&response)),
    (200, json!({"ok": true})),
    "{response}"
  );
  let (_, overview) = raw(
    live.addr,
    &format!(
      "GET /api/v1/overview HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
      live.addr
    ),
  );
  assert!(overview.contains("\"paused\":true"), "{overview}");
  let event = streamed(live.addr, "all by=web");
  assert!(event.contains("\"pause\""), "{event}");
  let reply = exchange_retry(&live.paths, PROTOCOL, &json!({"op": "pause"})).expect("cli pause");
  assert_eq!(reply, json!({"ok": true}));
  let rows = journal(&live.paths);
  assert!(
    rows.contains("all by=web") && rows.contains("all by=user"),
    "{rows}"
  );
  live.stop();
}

#[test]
fn each_op_replies_as_the_control_socket_does() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), crate::http_action::DEADLINE);
  let pairs = [
    (
      "resume",
      json!({"epic": "one"}),
      json!({"op": "resume", "epic": "one"}),
    ),
    (
      "ack",
      json!({"kind": "budget"}),
      json!({"op": "ack", "key": "budget"}),
    ),
    ("ack", json!({"key": "a"}), json!({"op": "ack", "key": "a"})),
    (
      "answer",
      json!({"key": "a", "text": "take B", "submission_id": "s-take-b"}),
      json!({"op": "answer", "key": "a", "text": "take B"}),
    ),
    (
      "answer",
      json!({"key": "zz", "text": "line\n\ttab", "submission_id": "s-zz"}),
      json!({"op": "answer", "key": "zz", "text": "x"}),
    ),
    (
      "release",
      json!({"key": "a"}),
      json!({"op": "release", "key": "a"}),
    ),
  ];
  let mut seen = String::new();
  for (op, web, socket) in pairs {
    let (code, response) = post(live.addr, op, &allowed(live.addr), &web.to_string());
    let cli = exchange_retry(&live.paths, PROTOCOL, &socket).expect("socket");
    let mut expected = cli.clone();
    if let Some(error) = cli.get("error") {
      assert_eq!(code, 409, "{op} {response}");
      expected = json!({"schema": 1, "error": error});
    } else {
      assert_eq!(code, 200, "{op} {response}");
    }
    assert_eq!(body(&response), expected, "{op}");
    seen.push_str(&response);
  }
  assert!(seen.contains("no live agent for zz") && seen.contains("a is not held"));
  let rows = journal(&live.paths);
  assert!(rows.contains("one by=web"), "{rows}");
  assert!(!seen.contains(TOKEN) && !token_on_disk(&live.root));
  live.stop();
}

#[test]
fn a_web_host_pause_and_resume_match_the_socket_and_journal_by_web() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), crate::http_action::DEADLINE);
  let (code, response) = post(
    live.addr,
    "host-pause",
    &allowed(live.addr),
    &json!({"host": "cursor-agent"}).to_string(),
  );
  assert_eq!(
    (code, body(&response)),
    (200, json!({"ok": true})),
    "{response}"
  );
  let (_, hosts) = raw(
    live.addr,
    &format!(
      "GET /api/v1/hosts HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
      live.addr
    ),
  );
  assert!(hosts.contains("\"paused\":true"), "{hosts}");
  assert!(hosts.contains("\"paused_by\":\"web\""), "{hosts}");
  let (code, response) = post(
    live.addr,
    "host-resume",
    &allowed(live.addr),
    &json!({"host": "cursor"}).to_string(),
  );
  assert_eq!(
    (code, body(&response)),
    (200, json!({"ok": true})),
    "{response}"
  );
  let rows = journal(&live.paths);
  assert!(rows.contains("host-paused"), "{rows}");
  assert!(rows.contains("by=web"), "{rows}");
  // An unknown or missing host is a 400 before anything is forwarded.
  for payload in [json!({"host": "nope"}).to_string(), "{}".to_owned()] {
    let (code, response) = post(live.addr, "host-pause", &allowed(live.addr), &payload);
    assert_eq!(code, 400, "{response}");
    assert!(
      response.contains("\"error\":\"invalid_request\""),
      "{response}"
    );
  }
  let rows_after = journal(&live.paths);
  assert_eq!(rows_after.matches("host-paused").count(), 1, "{rows_after}");
  live.stop();
}

/// A web `answer` body carrying `submission_id`.
fn answer_body(text: &str, submission_id: &str) -> String {
  json!({"key": "a", "text": text, "submission_id": submission_id}).to_string()
}

/// How many prompts the script recorded that hold `needle`.
fn prompted(live: &super::tests::Live, needle: &str) -> usize {
  std::fs::read_to_string(live.root.join("prompts"))
    .unwrap_or_default()
    .matches(needle)
    .count()
}

#[test]
fn an_uncertain_answer_is_ok_and_an_admitted_timeout_is_uncertain_not_busy() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), Duration::from_millis(200));
  let maybe = answer_body("MAYBE", "s-maybe");
  let (code, response) = post(live.addr, "answer", &allowed(live.addr), &maybe);
  assert_eq!(code, 200, "{response}");
  assert_eq!(body(&response)["delivered"], "uncertain");
  let slow = answer_body("SLOW", "s-slow");
  let start = Instant::now();
  let (code, response) = post(live.addr, "answer", &allowed(live.addr), &slow);
  assert_eq!(code, 200, "{response}");
  assert_eq!(
    body(&response),
    json!({"ok": true, "delivered": "uncertain",
      "note": "engine reply not confirmed before deadline"})
  );
  assert!(start.elapsed() < Duration::from_millis(900));
  std::thread::sleep(Duration::from_millis(1_300));
  assert_eq!(prompted(&live, "SLOW"), 1);
  // The retry of the same click, after the first finished, types nothing more.
  let (code, response) = post(live.addr, "answer", &allowed(live.addr), &slow);
  assert_eq!((code, body(&response)), (200, json!({"ok": true})));
  assert_eq!(prompted(&live, "SLOW"), 1);
  live.stop();
}

#[test]
fn the_same_submission_id_and_text_prompts_once() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), crate::http_action::DEADLINE);
  let once = answer_body("DUP", "s-dup");
  for _ in 0..2 {
    let (code, response) = post(live.addr, "answer", &allowed(live.addr), &once);
    assert_eq!((code, body(&response)), (200, json!({"ok": true})));
  }
  assert_eq!(prompted(&live, "DUP"), 1);
  let rows = journal(&live.paths);
  assert_eq!(rows.matches("answer-admit").count(), 1, "{rows}");
  live.stop();
}

#[test]
fn the_same_submission_id_with_other_text_is_a_409_and_prompts_nothing() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), crate::http_action::DEADLINE);
  let (code, response) = post(
    live.addr,
    "answer",
    &allowed(live.addr),
    &answer_body("FIRST", "s-mix"),
  );
  assert_eq!(code, 200, "{response}");
  let (code, response) = post(
    live.addr,
    "answer",
    &allowed(live.addr),
    &answer_body("SECOND", "s-mix"),
  );
  assert_eq!(code, 409, "{response}");
  assert_eq!(
    body(&response),
    json!({"schema": 1, "error": "submission_id reuse with different payload"})
  );
  assert_eq!(
    (prompted(&live, "FIRST"), prompted(&live, "SECOND")),
    (1, 0)
  );
  live.stop();
}

#[test]
fn a_new_submission_id_with_the_same_text_prompts_again() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), crate::http_action::DEADLINE);
  for id in ["s-one", "s-two"] {
    let (code, response) = post(
      live.addr,
      "answer",
      &allowed(live.addr),
      &answer_body("AGAIN", id),
    );
    assert_eq!((code, body(&response)), (200, json!({"ok": true})));
  }
  assert_eq!(prompted(&live, "AGAIN"), 2);
  live.stop();
}

fn status_of(response: &crate::http_response::Response) -> (u16, Value) {
  let body = serde_json::from_slice(&response.body).expect("json body");
  (response.status, body)
}

#[test]
fn only_a_request_the_state_loop_never_took_is_busy() {
  let request = || json!({"op": "pause", "by": "web"});
  let none = super::Control {
    sender: None,
    deadline: Duration::from_millis(50),
  };
  let (code, body) = status_of(&super::forward(&none, request()));
  assert_eq!((code, &body["error"]), (503, &json!("engine_busy")));
  // A closed state loop never admitted the line.
  let (closed, receiver) = std::sync::mpsc::channel();
  drop(receiver);
  let gone = super::Control {
    sender: Some(&closed),
    deadline: Duration::from_millis(50),
  };
  let (code, body) = status_of(&super::forward(&gone, request()));
  assert_eq!((code, &body["error"]), (503, &json!("engine_busy")));
  // An admitted line whose reply never comes, or whose reply handle is dropped.
  let (open, held) = std::sync::mpsc::channel();
  let silent = super::Control {
    sender: Some(&open),
    deadline: Duration::from_millis(50),
  };
  let uncertain = json!({"ok": true, "delivered": "uncertain",
    "note": "engine reply not confirmed before deadline"});
  assert_eq!(
    status_of(&super::forward(&silent, request())),
    (200, uncertain.clone())
  );
  let dropper = std::thread::spawn(move || {
    for message in held {
      drop(message);
    }
  });
  assert_eq!(
    status_of(&super::forward(&silent, request())),
    (200, uncertain)
  );
  drop(open);
  dropper.join().expect("dropper");
}
