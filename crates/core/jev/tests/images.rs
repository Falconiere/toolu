//! Images, Box answers and long context against the loopback endpoint: what
//! travels under `assets`, and which of `TypeSafe`'s opt-in headers a request sends.

#[path = "helpers/endpoint.rs"]
mod endpoint;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use endpoint::{Endpoint, PATH};
use serde_json::{Value, json};
use toolu_http_test_support::Reply;
use toolu_jev_client::{Answer, Assets, DEFAULT_MODEL, Extras, Questions, State};

fn answer() -> Value {
  json!({ "model": "jev-1.14.0", "answers": { "q": { "type": "noul", "noul": 0.92 } },
          "usage": { "input_tokens": 3, "output_tokens": 2 } })
}

/// One Noul question under `q`.
const NOUL: &str = r#"{"q":{"type":"noul","instructions":"Urgent?"}}"#;

/// The 16x16 PNG of `TypeSafe`'s Box example.
const PNG: &[u8] = b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x10\x00\x00\x00\x10\x08\x02\x00\x00\x00\x90\x91\x68\x36\x00\x00\x00\x1c\x49\x44\x41\x54\x78\x9c\x63\xf8\x4f\x22\x60\xa0\xa7\x06\x06\x06\xec\x68\x54\x03\x7d\x35\x10\x07\x86\x83\x06\x00\xc0\x7a\x7d\x9f\xa6\x2b\x6c\x54\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82";

/// The value of `TypeSafe` opt-in header `name` on each request received.
fn opted(endpoint: &Endpoint, name: &str) -> Vec<Option<String>> {
  let requests = endpoint.fixture.requests().unwrap_or_default();
  requests
    .iter()
    .map(|request| request.headers.get(name).cloned())
    .collect()
}

#[test]
fn ask_with_sends_images_under_assets_and_opts_in_by_header() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(200, answer().to_string()))
    .expect("route");
  let mut assets = Assets::default();
  assets.image("shot-7f3a", PNG).expect("image");
  let extras = Extras {
    assets,
    long_context: true,
  };
  endpoint
    .jev(&[])
    .expect("jev")
    .ask_with(
      &State::text("shot-7f3a"),
      DEFAULT_MODEL,
      &Questions::parse(NOUL).expect("noul"),
      &extras,
    )
    .expect("reply");
  assert_eq!(
    endpoint.bodies(),
    [format!(
      r#"{{"state":"shot-7f3a","model":"jev-latest","questions":{{"q":{{"type":"noul","instructions":"Urgent?"}}}},"assets":{{"shot-7f3a":{{"type":"image","format":"png","data_base64":"{}"}}}}}}"#,
      STANDARD.encode(PNG)
    )]
  );
  let on = [Some("true".to_owned())];
  assert_eq!(opted(&endpoint, "typesafe-danger-zone-images"), on);
  assert_eq!(opted(&endpoint, "typesafe-danger-zone-long-context"), on);
}

#[test]
fn a_box_question_turns_images_on_and_a_text_request_opts_into_nothing() {
  let endpoint = Endpoint::start().expect("endpoint");
  let jev = endpoint.jev(&[]).expect("jev");
  let located = json!({ "model": "jev-1.14.0", "usage": { "input_tokens": 290, "output_tokens": 21 },
    "answers": { "mug": { "type": "box", "confidence": null,
      "box": { "top": 0.25, "bottom": 0.82, "left": 0.44, "right": 0.84 } } } });
  endpoint
    .fixture
    .route(PATH, Reply::new(200, located.to_string()))
    .expect("route");
  let ask =
    Questions::parse(r#"{"mug":{"type":"box","instructions":"the mug in `photo`"}}"#).expect("ask");
  let reply = jev
    .ask(&State::text("photo"), DEFAULT_MODEL, &ask)
    .expect("box");
  assert_eq!(
    reply.answers,
    [(
      "mug".to_owned(),
      Answer::Box {
        top: 0.25,
        bottom: 0.82,
        left: 0.44,
        right: 0.84,
        confidence: None
      }
    )]
  );
  endpoint
    .fixture
    .route(PATH, Reply::new(200, answer().to_string()))
    .expect("route");
  jev
    .ask_with(
      &State::text("ticket"),
      DEFAULT_MODEL,
      &Questions::parse(NOUL).expect("noul"),
      &Extras::default(),
    )
    .expect("text");
  assert_eq!(
    opted(&endpoint, "typesafe-danger-zone-images"),
    [Some("true".to_owned()), None]
  );
  assert_eq!(
    opted(&endpoint, "typesafe-danger-zone-long-context"),
    [None, None]
  );
  assert_eq!(endpoint.last().get("assets"), None);
}
