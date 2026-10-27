//! Allowed actions on a real engine: the same replies as the control socket,
//! journaled `by=web`, and a busy engine answered `503` without a retry.

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
      json!({"key": "a", "text": "take B"}),
      json!({"op": "answer", "key": "a", "text": "take B"}),
    ),
    (
      "answer",
      json!({"key": "zz", "text": "line\n\ttab"}),
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
fn an_uncertain_answer_is_ok_and_a_busy_engine_is_never_retried() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), Duration::from_millis(200));
  let maybe = json!({"key": "a", "text": "MAYBE"}).to_string();
  let (code, response) = post(live.addr, "answer", &allowed(live.addr), &maybe);
  assert_eq!(code, 200, "{response}");
  assert_eq!(body(&response)["delivered"], "uncertain");
  let slow = json!({"key": "a", "text": "SLOW"}).to_string();
  let start = Instant::now();
  let (code, response) = post(live.addr, "answer", &allowed(live.addr), &slow);
  assert_eq!(code, 503, "{response}");
  assert!(response.contains("\"error\":\"engine_busy\""));
  assert!(start.elapsed() < Duration::from_millis(900));
  std::thread::sleep(Duration::from_millis(1_300));
  let prompts = std::fs::read_to_string(live.root.join("prompts")).expect("prompts");
  assert_eq!(prompts.matches("SLOW").count(), 1, "{prompts}");
  live.stop();
}
