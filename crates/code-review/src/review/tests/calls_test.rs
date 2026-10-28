use super::{Call, Ledger, Stage, jev_failure};
use crate::contract::Cost;

fn call(stage: Stage, tokens: (u64, u64), cost_usd: Option<f64>) -> Call {
  Call {
    stage,
    model: "m".into(),
    provider: None,
    quantization: None,
    tokens_in: tokens.0,
    tokens_out: tokens.1,
    cost_usd,
    millis: 1,
    outcome: "ok".into(),
  }
}

#[test]
fn usage_sums_tokens_and_the_costs_that_were_reported() {
  let mut ledger = Ledger::new(None);
  assert_eq!(ledger.usage(None).cost_usd, None);
  ledger
    .calls
    .push(call(Stage::Review, (100, 50), Some(0.25)));
  ledger.calls.push(call(Stage::Judge, (10, 2), None));
  ledger.calls.push(call(Stage::Recheck, (5, 5), Some(0.5)));
  let usage = ledger.usage(Some("deepseek".into()));
  assert_eq!((usage.tokens_in, usage.tokens_out), (115, 57));
  assert_eq!(usage.cost_usd, Cost::usd(0.75));
  assert_eq!(usage.model.as_deref(), Some("deepseek"));
}

#[test]
fn a_jev_failure_is_described_without_its_body() {
  let http = toolu_jev_client::Error::Http {
    status: 503,
    body: "provider said secret things".into(),
  };
  assert_eq!(jev_failure(&http), "HTTP 503");
  assert_eq!(jev_failure(&toolu_jev_client::Error::Timeout), "deadline");
  assert_eq!(
    jev_failure(&toolu_jev_client::Error::InvalidResponse),
    "invalid response"
  );
  let doc = serde_json::to_value(call(Stage::Selector, (1, 1), None)).unwrap();
  assert_eq!(doc["stage"], "selector");
}
