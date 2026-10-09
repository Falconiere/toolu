use super::{kind, returned_bytes, session_id};

#[test]
fn savings_response_projection_matches_every_recorded_fixture_case() {
  let cases: serde_json::Value =
    serde_json::from_str(include_str!("../../../../fixtures/ast-grep/savings.json"))
      .expect("savings fixture");
  let golden: serde_json::Value =
    serde_json::from_str(include_str!("../../../../fixtures/ast-grep/golden.json"))
      .expect("savings golden");
  for case in cases["cases"].as_array().expect("cases") {
    let name = case["name"].as_str().expect("name");
    let payload = &case["payload"];
    let tool = payload["tool_name"].as_str().expect("tool name");
    let input = payload["tool_input"].as_object().expect("tool input");
    if case["deviation"]["silent"] == true {
      assert_eq!(kind(tool, input), None, "{name}");
      continue;
    }
    if case["deviation"]["contains"].is_string() {
      assert_eq!(kind(tool, input), Some("ast-grep"), "{name}");
      continue;
    }
    let key = format!("{name} [claude]");
    let ledgers = golden["savings"][&key]["ledgers"]
      .as_object()
      .expect("golden ledgers");
    let Some((_, line)) = ledgers.iter().next() else {
      continue;
    };
    let expected: serde_json::Value =
      serde_json::from_str(line.as_str().expect("ledger line")).expect("recorded line");
    let actual = returned_bytes(payload.get("tool_response"));
    assert_eq!(
      actual,
      expected["returned"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok()),
      "{name}"
    );
    assert_eq!(kind(tool, input), expected["kind"].as_str(), "{name}");
  }
}

#[test]
fn savings_identifies_actual_ast_grep_command_and_response() {
  let input = serde_json::json!({"command":"ast-grep run -p 'console.log($A)' -l typescript src"});
  assert_eq!(
    kind("Bash", input.as_object().expect("object")),
    Some("ast-grep")
  );
  let response = serde_json::json!({"output":"match line\n"});
  assert_eq!(returned_bytes(Some(&response)), Some(10));
}

#[test]
fn savings_sanitizes_session_and_ignores_empty_result() {
  assert_eq!(
    session_id(Some(&serde_json::json!("ses_opencode-1"))),
    "sesopencode-1"
  );
  assert_eq!(returned_bytes(Some(&serde_json::json!(""))), None);
}
