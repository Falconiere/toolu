use toolu_runtime::json::ordered::Ordered;

use super::{parse_telemetry_extras, parse_telemetry_line};
use crate::telemetry::TelemetryEvent;

fn json(text: &str) -> Ordered {
  Ordered::parse(text).unwrap()
}

const PROTOCOL: &str = "\"v\":1,\"t\":\"2026-01-01T00:00:00Z\",\"branch\":\"feat/x\"";

#[test]
fn a_line_reads_its_event_extras_and_protocol_fields() {
  let line = json(&format!(
    "{{\"result\":\"deny\",\"reason_code\":\"stale\",\"round\":null,{PROTOCOL},\"event\":\"push_check\"}}"
  ));
  let parsed = parse_telemetry_line(&line).unwrap();
  assert_eq!(
    parsed.event,
    TelemetryEvent::PushCheck {
      result: "deny".into(),
      reason_code: "stale".into(),
      round: None
    }
  );
  assert_eq!(
    (parsed.t.as_str(), parsed.branch.as_str()),
    ("2026-01-01T00:00:00Z", "feat/x")
  );
  let delegation = json(&format!(
    "{{\"model\":\"sonnet\",\"subagent_type\":null,\"reasoning_effort\":null,\"step_id\":\"S1\",\"step_model\":null,{PROTOCOL},\"event\":\"delegation\"}}"
  ));
  assert!(matches!(
    parse_telemetry_line(&delegation).unwrap().event,
    TelemetryEvent::Delegation { .. }
  ));
}

#[test]
fn a_line_outside_the_schema_is_rejected_with_its_key() {
  let cases = [
    (
      format!("{{{PROTOCOL},\"event\":\"page_view\"}}"),
      "event: unknown event \"page_view\"",
    ),
    (
      format!(
        "{{\"file\":\"a\",\"source\":\"b\",\"cmd\":\"rm\",{PROTOCOL},\"event\":\"gate_fail\"}}"
      ),
      "(root): unrecognized key \"cmd\"",
    ),
    (
      format!("{{\"file\":{{\"x\":1}},\"source\":\"b\",{PROTOCOL},\"event\":\"gate_fail\"}}"),
      "file: expected string",
    ),
    (
      "{\"v\":2,\"t\":\"t\",\"branch\":\"b\",\"event\":\"docs_nudge\"}".to_owned(),
      "v: expected 1",
    ),
    (
      "{\"v\":1,\"t\":\"t\",\"event\":\"docs_nudge\"}".to_owned(),
      "branch: required",
    ),
    (
      format!("{{\"covered\":\"3\",\"uncovered\":1,{PROTOCOL},\"event\":\"ac_coverage\"}}"),
      "covered: expected number",
    ),
    (
      format!(
        "{{\"result\":\"a\",\"reason_code\":\"b\",\"round\":\"1\",{PROTOCOL},\"event\":\"push_check\"}}"
      ),
      "round: expected number or null",
    ),
    (
      format!(
        "{{\"model\":1,\"subagent_type\":null,\"reasoning_effort\":null,\"step_id\":null,\"step_model\":null,{PROTOCOL},\"event\":\"delegation\"}}"
      ),
      "model: expected string or null",
    ),
    ("[1]".to_owned(), "(root): expected object"),
  ];
  for (text, error) in cases {
    assert_eq!(
      parse_telemetry_line(&json(&text)),
      Err(error.to_owned()),
      "{text}"
    );
  }
}

#[test]
fn extras_reject_protocol_keys_smuggled_in_by_a_caller() {
  assert_eq!(
    parse_telemetry_extras("docs_nudge", &json("{\"event\":\"gate_clear\"}")),
    Err("(root): unrecognized key \"event\"".to_owned())
  );
  assert!(
    parse_telemetry_extras(
      "gate_fail",
      &json("{\"file\":\"a\",\"source\":\"b\",\"branch\":\"main\"}")
    )
    .is_err()
  );
  let run =
    json("{\"step_id\":\"S\",\"status\":\"ok\",\"exit_code\":0,\"duration_s\":1.5,\"attempt\":1}");
  assert!(matches!(
    parse_telemetry_extras("step_run", &run),
    Ok(TelemetryEvent::StepRun { .. })
  ));
  assert!(parse_telemetry_extras("docs_attested", &json("{\"decision\":\"updated\"}")).is_ok());
}
