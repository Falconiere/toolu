use std::path::PathBuf;

use serde_json::json;
use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickDecision, TickReport, TickRequest};

use crate::babysit::{Next, next};

/// A tick that answers with a fixed outcome.
struct Answer(Result<TickDecision, LinkError>);

impl BabysitTick for Answer {
  fn tick(&self, request: &TickRequest) -> Result<TickReport, LinkError> {
    self.0.clone().map(|decision| TickReport {
      decision,
      result: json!({ "slot": format!("{}#{}", request.repo, request.number) }),
    })
  }
}

fn request() -> TickRequest {
  TickRequest {
    repo: "Falconiere/toolu".into(),
    number: 460,
    state_file: PathBuf::from("/state/460.json"),
    now: None,
  }
}

#[test]
fn each_decision_becomes_the_engines_next_step() {
  let cases = [
    (Ok(TickDecision::KeepGoing), Next::KeepGoing),
    (Ok(TickDecision::Success), Next::MergeQueue),
    (
      Ok(TickDecision::Escalate),
      Next::Attention("babysit escalated Falconiere/toolu#460".into()),
    ),
  ];
  for (answer, expected) in cases {
    assert_eq!(next(&Answer(answer), &request()), expected);
  }
}

#[test]
fn a_failed_tick_is_an_attention_item_not_a_crash() {
  let failed = Answer(Err(LinkError::Failed("no GitHub token".into())));
  assert_eq!(
    next(&failed, &request()),
    Next::Attention("babysit tick for Falconiere/toolu#460 failed: no GitHub token".into())
  );
  let unported = Answer(Err(LinkError::NotPorted { issue: 433 }));
  assert_eq!(
    next(&unported, &request()),
    Next::Attention("babysit tick for Falconiere/toolu#460 failed: not ported yet (#433)".into())
  );
}
