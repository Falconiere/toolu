use super::{LenientString, Payload, parse};
use crate::host::Host;

#[test]
fn each_host_parses_into_its_own_payload_type() {
  let doc = r#"{"hook_event_name":"PreToolUse","tool_name":"Bash"}"#;
  assert!(matches!(parse(Host::Claude, doc), Ok(Payload::Claude(_))));
  assert!(matches!(parse(Host::Codex, doc), Ok(Payload::Codex(_))));
  assert!(matches!(parse(Host::Cursor, doc), Ok(Payload::Cursor(_))));
  assert!(matches!(parse(Host::Hermes, doc), Ok(Payload::Hermes(_))));
  assert!(matches!(
    parse(Host::Opencode, doc),
    Ok(Payload::Opencode(_))
  ));
}

#[test]
fn an_empty_object_parses_with_every_field_absent() {
  let Ok(Payload::Claude(payload)) = parse(Host::Claude, "{}") else {
    panic!("{{}} did not parse");
  };
  assert_eq!(payload.tool_name, LenientString::default());
  assert!(payload.tool_input.is_none() && payload.rest.is_empty());
}

#[test]
fn anything_but_a_json_object_is_an_error() {
  for text in ["", "not json\n", "{not json", "[]", "7", "\"text\"", "null"] {
    for host in Host::ALL {
      assert!(parse(host, text).is_err(), "{host:?} {text:?}");
    }
  }
}

#[test]
fn a_duplicate_named_key_and_a_lone_surrogate_are_errors() {
  let duplicate = r#"{"tool_name":"Bash","tool_name":"Edit"}"#;
  let err = parse(Host::Claude, duplicate).err().unwrap();
  assert!(err.to_string().contains("duplicate field"), "{err}");
  assert!(parse(Host::Claude, r#"{"prompt":"\ud800"}"#).is_err());
}

#[test]
fn a_lenient_string_reads_a_string_and_drops_any_other_value() {
  let read = |json: &str| serde_json::from_str::<LenientString>(json).unwrap();
  assert_eq!(read(r#""Bash""#).as_str(), Some("Bash"));
  assert_eq!(read(r#""""#).as_str(), Some(""));
  for other in ["7", "false", "null", "{}", "[\"a\"]"] {
    assert_eq!(read(other).as_str(), None, "{other}");
  }
}

#[test]
fn text_is_a_non_empty_string_like_typescript_text_with_a_fallback() {
  let read = |json: &str| serde_json::from_str::<LenientString>(json).unwrap();
  assert_eq!(read(r#""s1""#).text(), Some("s1"));
  assert_eq!(read(r#""""#).text(), None);
  assert_eq!(read("7").text(), None);
}
