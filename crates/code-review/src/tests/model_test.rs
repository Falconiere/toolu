use serde_json::{Value, json};
use toolu_openrouter::{Completion, Usage};

use crate::budget::Effort;
use crate::contract::Verdict;

use super::{ModelError, SCHEMA_JSON, error_report, parse, request, usage};

fn completion(content: &str, finish_reason: &str) -> Completion {
  Completion {
    content: content.to_owned(),
    finish_reason: Some(finish_reason.to_owned()),
    model: "deepseek/deepseek-v4-pro-0813".into(),
    provider: Some("Parasail".into()),
    usage: Usage {
      prompt_tokens: 1,
      completion_tokens: 10,
      reasoning_tokens: 5,
      cost_usd: None,
    },
  }
}

fn finding() -> Value {
  json!({"path":"src/lib.rs","line":4,"end_line":4,"side":"new","severity":"high","category":"correctness","claim":"This can panic","quote":"value[0]","suggestion":null,"prior_id":null})
}

#[test]
fn committed_schema_accepts_the_valid_model_shape() {
  let schema: Value = serde_json::from_str(SCHEMA_JSON).expect("schema");
  let validator = jsonschema::validator_for(&schema).expect("validator");
  assert!(validator.is_valid(&json!({"findings":[finding()]})));
  assert!(!validator.is_valid(&json!({"findings":[{"path":"x"}]})));
  assert!(!validator.is_valid(&json!({"verdict":"approved","findings":[]})));
}

#[test]
fn review_request_maps_effort_to_reasoning_and_the_completion_ceiling() {
  let messages = [toolu_openrouter::Message {
    role: "user",
    content: "review",
  }];
  let schema: Value = serde_json::from_str(SCHEMA_JSON).expect("schema");
  for effort in [
    Effort::Small,
    Effort::Medium,
    Effort::High,
    Effort::Xhigh,
    Effort::Auto,
  ] {
    let call = request(&messages, &schema, effort);
    assert_eq!(call.max_tokens, effort.budget().max_tokens);
    assert_eq!(call.reasoning_effort, effort.budget().reasoning);
    assert!(call.max_tokens <= Effort::Xhigh.budget().max_tokens);
    assert_eq!(call.schema_name, "code_review");
  }
  assert_eq!(
    crate::budget::HTTP_RESPONSE_MAX_BYTES,
    toolu_openrouter::MAX_BODY_BYTES
  );
}

#[test]
fn complete_findings_and_a_valid_empty_array_are_accepted() {
  let parsed = parse(&completion(
    &json!({"findings":[finding()]}).to_string(),
    "stop",
  ))
  .expect("findings");
  assert_eq!(parsed.findings.len(), 1);
  assert!(!parsed.partial);
  assert_eq!(parsed.dropped, 0);
  let empty = parse(&completion("{\"findings\":[]}", "stop")).expect("empty");
  assert_eq!(empty.findings.len(), 0);
  assert!(!empty.partial);
}

#[test]
fn truncated_array_keeps_only_closed_valid_findings() {
  let content = format!(
    "{{\"findings\":[{},{{\"path\":\"src/bad.rs\",\"line\":",
    finding()
  );
  let parsed = parse(&completion(&content, "length")).expect("salvaged");
  assert_eq!(parsed.findings.len(), 1);
  assert_eq!(parsed.findings[0].quote, "value[0]");
  assert!(parsed.partial);
  assert_eq!(parsed.dropped, 1);
  let between = format!("{{\"findings\":[{},", finding());
  let parsed = parse(&completion(&between, "length")).expect("salvaged");
  assert_eq!(parsed.findings.len(), 1);
  assert!(parsed.partial);
  assert_eq!(parsed.dropped, 0);
}

#[test]
fn schema_valid_whitespace_preserves_exact_evidence_without_marking_partial() {
  let mut value = finding();
  value["quote"] = json!("    let x = value[0];");
  value["suggestion"] = json!("use first()\n");
  let parsed = parse(&completion(
    &json!({"findings":[value]}).to_string(),
    "stop",
  ))
  .expect("finding");
  assert_eq!(parsed.findings[0].quote, "    let x = value[0];");
  assert_eq!(
    parsed.findings[0].suggestion.as_deref(),
    Some("use first()\n")
  );
  assert!(!parsed.partial);
}

#[test]
fn path_and_blank_optional_fields_normalize_without_changing_evidence() {
  let mut value = finding();
  value["path"] = json!(" src/lib.rs ");
  value["quote"] = json!("  value[0] ");
  value["suggestion"] = json!("");
  value["prior_id"] = json!(" ");
  let parsed = parse(&completion(
    &json!({"findings":[value]}).to_string(),
    "stop",
  ))
  .expect("finding");
  assert!(parsed.partial);
  assert_eq!(parsed.findings[0].path, "src/lib.rs");
  assert_eq!(parsed.findings[0].quote, "  value[0] ");
  assert_eq!(parsed.findings[0].suggestion, None);
  assert_eq!(parsed.findings[0].prior_id, None);
}

#[test]
fn extra_top_level_fields_and_abnormal_finishes_never_approve_empty_findings() {
  let extra = completion("{\"findings\":[],\"summary\":\"ok\"}", "stop");
  assert!(matches!(parse(&extra), Err(ModelError::Malformed { .. })));
  let filtered = completion("{\"findings\":[]}", "content_filter");
  assert!(matches!(
    parse(&filtered),
    Err(ModelError::Malformed { .. })
  ));
  let mut missing_reason = completion("{\"findings\":[]}", "stop");
  missing_reason.finish_reason = None;
  assert!(matches!(
    parse(&missing_reason),
    Err(ModelError::Malformed { .. })
  ));
  let with_finding = completion(
    &json!({"findings":[finding()]}).to_string(),
    "content_filter",
  );
  assert!(parse(&with_finding).expect("partial findings").partial);
}

#[test]
fn deviations_are_normalized_without_inventing_required_fields() {
  let mut normalized = finding();
  normalized["side"] = json!("NEW");
  normalized["severity"] = json!("HIGH");
  normalized["line"] = json!("4");
  normalized["quote"] = json!(" value[0] ");
  let invalid = json!({"path":"src/bad.rs","line":3,"end_line":3,"side":"new","severity":"high","category":"correctness","claim":"claim","suggestion":null,"prior_id":null});
  let parsed = parse(&completion(
    &json!({"findings":[normalized,invalid]}).to_string(),
    "stop",
  ))
  .expect("partial");
  assert_eq!(parsed.findings.len(), 1);
  assert_eq!(parsed.findings[0].quote, " value[0] ");
  assert!(parsed.partial);
  assert_eq!(parsed.dropped, 1);
}

#[test]
fn garbage_and_length_without_findings_are_error_verdicts_with_finish_reason() {
  for (content, reason) in [("garbage", "stop"), ("{\"findings\":[", "length")] {
    let err = parse(&completion(content, reason)).expect_err("error");
    assert!(matches!(err, ModelError::Malformed { .. }));
    let report = err.report(Effort::High);
    assert_eq!(report.verdict(), Verdict::Error);
    assert_eq!(report.usage.tokens_out, 10);
    assert_eq!(
      report.usage.model.as_deref(),
      Some("deepseek/deepseek-v4-pro-0813")
    );
    assert!(report.error.expect("error").message.contains(reason));
  }
}

#[test]
fn transport_errors_map_to_error_verdict_and_cost_stays_unknown() {
  let configuration = error_report(&toolu_openrouter::Error::InvalidConfig, Effort::High);
  assert_eq!(
    configuration.error.expect("error").kind,
    crate::contract::ErrorKind::Unconfigured
  );
  let report = error_report(
    &toolu_openrouter::Error::Truncated {
      finish_reason: Some("length".into()),
      usage: Some(Usage {
        prompt_tokens: 12,
        completion_tokens: 100,
        reasoning_tokens: 100,
        cost_usd: Some(0.5),
      }),
    },
    Effort::High,
  );
  assert_eq!(report.verdict(), Verdict::Error);
  assert_eq!(report.usage.tokens_out, 100);
  assert_eq!(
    serde_json::to_value(&report).expect("report")["usage"]["cost_usd"],
    0.5
  );
  assert!(report.error.expect("error").message.contains("length"));
  let completion = completion("{\"findings\":[]}", "stop");
  assert_eq!(usage(&completion).cost_usd, None);
}
