use std::time::Instant;

use serde_json::Value;
use toolu_runtime::config::secrets::Secrets;

use super::{Sample, Screen};
use crate::fleet_rig::secrets_of;
use crate::http_live::Herdr;

#[test]
fn cached_raw_screen_uses_the_current_secrets_and_blocked_attach_target() {
  let root = tempfile::tempdir().expect("root");
  let first = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", "FIRST-CANARY")]);
  let second = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", "SECOND-CANARY")]);
  let screen = Screen::new(Herdr::Off, Some("probe session".to_owned()));
  let sample = Sample {
    at: Instant::now(),
    sampled_at: "2026-10-27T00:00:00.000Z".to_owned(),
    target: "worker".to_owned(),
    result: Ok("FIRST-CANARY SECOND-CANARY".to_owned()),
  };
  let first_body: Value =
    serde_json::from_slice(&screen.render("key", "blocked", &sample, &first).body)
      .expect("first JSON");
  let second_body: Value =
    serde_json::from_slice(&screen.render("key", "blocked", &sample, &second).body)
      .expect("second JSON");
  assert_eq!(first_body["text"], "<redacted> SECOND-CANARY");
  assert_eq!(second_body["text"], "FIRST-CANARY <redacted>");
  assert_eq!(
    first_body["attach"],
    "herdr --session 'probe session' agent attach worker"
  );
  assert_eq!(first_body["agent_status"], "blocked");
}

#[test]
fn escaped_screen_text_keeps_its_tail_within_the_json_cap() {
  let screen = Screen::new(Herdr::Off, None);
  let sample = Sample {
    at: Instant::now(),
    sampled_at: "2026-10-27T00:00:00.000Z".to_owned(),
    target: "worker".to_owned(),
    result: Ok(format!("{}TAIL-Ω", "\0".repeat(20_000))),
  };
  let response = screen.render("key", "working", &sample, &Secrets::none());
  assert_eq!(response.status, 200);
  assert!(response.body.len() <= 65_536);
  let body: Value = serde_json::from_slice(&response.body).expect("JSON");
  let text = body["text"].as_str().expect("screen text");
  assert!(text.starts_with('\0') && text.ends_with("TAIL-Ω"));
  assert_eq!(body["lines"], 1);
}
