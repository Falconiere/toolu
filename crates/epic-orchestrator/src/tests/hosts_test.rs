use serde_json::Value;

use super::{
  AgentOpts, HOST_KINDS, HostKind, agent_args, host_kind, host_limited, parse_host_kind, skill_ref,
};

const FIXTURE: &str = include_str!("../../../../fixtures/epic/hosts.json");

fn cases(kind: &str) -> Vec<Value> {
  let doc: Value = serde_json::from_str(FIXTURE).expect("fixture");
  doc["cases"]
    .as_array()
    .expect("cases")
    .iter()
    .filter(|case| case["kind"] == kind)
    .cloned()
    .collect()
}

fn text(value: &Value) -> Option<String> {
  value.as_str().map(str::to_owned)
}

fn kind_of(case: &Value) -> HostKind {
  parse_host_kind(case["host"].as_str().expect("host")).expect("kind")
}

#[test]
fn agent_args_match_every_captured_combination() {
  let args = cases("args");
  assert!(args.len() >= 24 + 9);
  for case in args {
    let opts = &case["opts"];
    let options = AgentOpts {
      key: text(&opts["key"]).unwrap_or_default(),
      model: text(&opts["model"]),
      effort: text(&opts["effort"]),
      bypass: opts["bypass"].as_bool().unwrap_or(false),
      permission_mode: text(&opts["permissionMode"]).unwrap_or_default(),
      resume: opts["resume"].as_bool().unwrap_or(false),
      session_id: text(&opts["sessionId"]),
    };
    let expected: Vec<String> = serde_json::from_value(case["argv"].clone()).expect("argv");
    assert_eq!(
      agent_args(kind_of(&case), &options),
      Ok(expected),
      "{}",
      case["name"]
    );
  }
}

#[test]
fn skill_syntax_matches_every_host() {
  for case in cases("skill") {
    let got = skill_ref(
      kind_of(&case),
      case["plugin"].as_str().expect("plugin"),
      case["skill"].as_str().expect("skill"),
    );
    assert_eq!(Some(got), text(&case["text"]), "{}", case["name"]);
  }
}

#[test]
fn aliases_resolve_as_captured() {
  for case in cases("alias") {
    let got = parse_host_kind(case["alias"].as_str().expect("alias")).map(HostKind::name);
    assert_eq!(
      got.map(str::to_owned),
      text(&case["host"]),
      "{}",
      case["name"]
    );
  }
  assert_eq!(
    host_kind("gemini").err().as_deref(),
    Some("unknown host gemini; use one of claude, codex, cursor")
  );
  assert_eq!(parse_host_kind("opencode"), None);
  assert_eq!(
    HOST_KINDS.map(HostKind::name),
    ["claude", "codex", "cursor"]
  );
}

#[test]
fn the_limit_pattern_matches_throttle_messages_not_ordinary_output() {
  for case in cases("limit") {
    let text = case["text"].as_str().expect("text");
    assert_eq!(
      host_limited(text),
      case["matches"].as_bool().expect("matches"),
      "{text}"
    );
  }
}

#[test]
fn unsafe_arguments_are_refused() {
  let opts = AgentOpts {
    key: "k".into(),
    model: Some("bad model".into()),
    bypass: true,
    ..AgentOpts::default()
  };
  assert_eq!(
    agent_args(HostKind::Claude, &opts).err().as_deref(),
    Some("claude arg the pane shell would mangle: bad model")
  );
  let cursor = AgentOpts {
    key: "k".into(),
    model: Some("claude[effort=high]".into()),
    bypass: true,
    ..AgentOpts::default()
  };
  assert!(agent_args(HostKind::Cursor, &cursor).is_err());
}
