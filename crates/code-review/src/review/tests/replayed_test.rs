use super::{REPLAY_MISS, REPLAYED, replay_failure, restored};

#[test]
fn a_recorded_completion_is_rebuilt_and_charged_nothing() {
  let body = r#"{"content":"{\"findings\":[]}","finish_reason":"stop","model":"m/served",
    "provider":"Parasail","provider_quantization":"fp8",
    "usage":{"prompt_tokens":120,"completion_tokens":30,"reasoning_tokens":5,"cost":0.25}}"#;
  let done = restored(body).expect("a recorded completion");
  assert_eq!(done.content, "{\"findings\":[]}");
  assert_eq!(done.finish_reason.as_deref(), Some("stop"));
  assert_eq!(
    (done.model.as_str(), done.provider.as_deref()),
    ("m/served", Some("Parasail"))
  );
  assert_eq!(done.quantization.as_deref(), Some("fp8"));
  assert_eq!(
    (
      done.usage.prompt_tokens,
      done.usage.completion_tokens,
      done.usage.reasoning_tokens
    ),
    (120, 30, 5)
  );
  assert_eq!(done.usage.cost_usd, Some(0.0));
}

#[test]
fn a_recorded_completion_without_its_essentials_is_invalid() {
  for body in [
    "not json",
    r#"{"model":"m","usage":{"prompt_tokens":1,"completion_tokens":1}}"#,
    r#"{"content":"x","usage":{"prompt_tokens":1,"completion_tokens":1}}"#,
    r#"{"content":"x","model":"m","usage":{"prompt_tokens":1}}"#,
  ] {
    assert_eq!(
      restored(body).err(),
      Some(toolu_openrouter::Error::InvalidResponse),
      "{body}"
    );
  }
}

#[test]
fn only_a_missing_request_is_described_as_a_replay_miss() {
  assert_eq!(
    replay_failure(&toolu_openrouter::Error::InvalidConfig),
    REPLAY_MISS
  );
  assert_eq!(
    replay_failure(&toolu_openrouter::Error::InvalidResponse),
    "OpenRouter returned an invalid response envelope"
  );
  assert_ne!(REPLAYED, REPLAY_MISS);
}
