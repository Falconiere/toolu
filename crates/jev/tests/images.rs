//! `--image`, Box answers and `--long-context` against the loopback endpoint.

#[path = "helpers/harness.rs"]
mod harness;

use harness::{ANSWER, Harness};
use serde_json::{Value, json};
use toolu_http_test_support::Reply;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

fn succeeded(outcome: &Outcome) {
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  assert_eq!(outcome.stderr, None);
}

/// The 16x16 PNG of `TypeSafe`'s Box example: 85 bytes, so 116 base64 characters.
const PNG: &[u8] = b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x10\x00\x00\x00\x10\x08\x02\x00\x00\x00\x90\x91\x68\x36\x00\x00\x00\x1c\x49\x44\x41\x54\x78\x9c\x63\xf8\x4f\x22\x60\xa0\xa7\x06\x06\x06\xec\x68\x54\x03\x7d\x35\x10\x07\x86\x83\x06\x00\xc0\x7a\x7d\x9f\xa6\x2b\x6c\x54\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82";

/// `TypeSafe`'s two opt-in headers on the first request: images, long context.
fn opted(harness: &Harness) -> (Option<String>, Option<String>) {
  let requests = harness.fixture.requests().unwrap_or_default();
  let header = |name: &str| requests.first()?.headers.get(name).cloned();
  (
    header("typesafe-danger-zone-images"),
    header("typesafe-danger-zone-long-context"),
  )
}

#[test]
fn an_image_travels_under_assets_and_turns_images_on() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  let dir = tempfile::tempdir().expect("test setup");
  let file = dir.path().join("shot.png");
  std::fs::write(&file, PNG).expect("test setup");
  let image = format!("shot-7f3a={}", file.display());
  let args = [
    "noul",
    "-s",
    "shot-7f3a",
    "Is an error dialog visible?",
    "-i",
    &image,
  ];
  succeeded(&harness.run(&args, Some("fixture-key"), None, &[]));
  let sent = harness.last();
  assert_eq!(sent["state"], "shot-7f3a");
  let asset = &sent["assets"]["shot-7f3a"];
  assert_eq!(
    (&asset["type"], &asset["format"]),
    (&json!("image"), &json!("png"))
  );
  let data = asset["data_base64"].as_str().unwrap_or_default();
  assert!(
    data.starts_with("iVBORw0KGgo") && data.ends_with("=="),
    "{data}"
  );
  assert_eq!(data.len(), 116);
  assert_eq!(
    sent["assets"].as_object().map(serde_json::Map::len),
    Some(1)
  );
  assert_eq!(opted(&harness), (Some("true".to_owned()), None));
}

#[test]
fn ask_prints_a_box_answer_and_long_context_sends_its_header() {
  let reply = json!({ "model": "jev-1.14.0", "usage": { "input_tokens": 290, "output_tokens": 21 },
    "answers": { "mug": { "type": "box", "confidence": null,
      "box": { "top": 0.25, "bottom": 0.82, "left": 0.44, "right": 0.84 } } } });
  let harness = Harness::start(vec![Reply::new(200, reply.to_string())]).expect("fixture");
  let outcome = harness.run(
    &["ask", "-", "-s", "photo", "--long-context"],
    Some("fixture-key"),
    Some(r#"{"mug":{"type":"box","instructions":"the mug in `photo`"}}"#),
    &[],
  );
  succeeded(&outcome);
  let printed: Value =
    serde_json::from_str(&outcome.stdout.expect("test setup")).expect("test setup");
  assert_eq!(printed, reply["answers"]);
  assert_eq!(harness.last().get("assets"), None);
  let on = Some("true".to_owned());
  assert_eq!(opted(&harness), (on.clone(), on));
}

#[test]
fn a_bad_image_fails_with_its_reason_before_any_request() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  let dir = tempfile::tempdir().expect("test setup");
  let notes = dir.path().join("notes.txt");
  std::fs::write(&notes, "not an image").expect("test setup");
  let text = format!("shot={}", notes.display());
  for (image, stderr) in [
    ("shot", "jev: --image needs REF=PATH"),
    (
      "shot=/nonexistent/shot.png",
      "jev: cannot read /nonexistent/shot.png",
    ),
    (text.as_str(), "jev: image shot is not PNG or JPEG"),
  ] {
    let args = ["noul", "-s", "shot", "Urgent?", "-i", image];
    let outcome = harness.run(&args, Some("fixture-key"), None, &[]);
    assert_eq!(
      (outcome.exit, outcome.stdout, outcome.stderr.as_deref()),
      (Exit::Failure, None, Some(stderr))
    );
  }
  assert_eq!(harness.bodies().len(), 0);
}
