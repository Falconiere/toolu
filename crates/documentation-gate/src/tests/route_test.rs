use super::{Candidate, select, validate};
use std::collections::BTreeMap;
use toolu_jev_client::{Answer, Reply, Usage};

fn candidate() -> Candidate {
  Candidate {
    id: "worker".into(),
    host: "codex".into(),
    model: "configured".into(),
    effort: "high".into(),
    description: "documentation worker".into(),
  }
}

fn reply(answer: Answer) -> Reply {
  Reply {
    model: "jev".into(),
    usage: Usage {
      input_tokens: 0,
      output_tokens: 0,
    },
    answers: vec![("route".into(), answer)],
    body: String::new(),
  }
}

#[test]
fn validates_profiles_and_choices() {
  let c = candidate();
  assert!(validate(&[]).is_err());
  assert!(validate(&[candidate(), candidate()]).is_err());
  assert!(
    validate(&[Candidate {
      id: "../bad".into(),
      ..candidate()
    }])
    .is_err()
  );
  assert!(
    validate(&[Candidate {
      host: "cursor".into(),
      ..candidate()
    }])
    .is_err()
  );
  assert!(validate(&[candidate()]).is_ok());
  let choice = |id: &str| Answer::Choice {
    choice: id.into(),
    probabilities: BTreeMap::new(),
    confidence: 1.0,
  };
  assert!(select(&[candidate()], &reply(choice("no-suitable-route"))).is_err());
  assert!(select(&[candidate()], &reply(Answer::Noul { noul: 1.0 })).is_err());
  let value = select(&[c], &reply(choice("worker")));
  assert!(value.is_ok_and(|v| v["candidate_id"] == "worker" && v["model"] == "configured"));
}
