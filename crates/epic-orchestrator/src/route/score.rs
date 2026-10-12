//! Jev complexity scores, in process (#460), for the issues that have no route.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use toolu_jev_client::{Answer, Jev, Questions, State};

use super::tier::{Evidence, LEVELS, tier_from_score};

/// One issue's tier and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Score {
  /// Tier index.
  pub tier: usize,
  /// Jev's expected score.
  pub expected: Option<f64>,
  /// Jev's confidence.
  pub confidence: Option<f64>,
  /// `jev` or `heuristic`.
  pub source: String,
}

fn questions(issues: &[Evidence]) -> Value {
  let mut out = serde_json::Map::new();
  for issue in issues {
    out.insert(
      issue.key.clone(),
      json!({
        "type": "score",
        "instructions": format!(
          "How much implementation complexity does `issues.{}` carry for one coding agent? Judge from its title, labels, description excerpt, and dependency counts only.",
          issue.key
        ),
        "criteria": LEVELS,
      }),
    );
  }
  Value::Object(out)
}

fn state(issues: &[Evidence]) -> Value {
  let mut out = serde_json::Map::new();
  for issue in issues {
    out.insert(
      issue.key.clone(),
      json!({
        "title": issue.title,
        "labels": issue.labels,
        "description_excerpt": issue.excerpt,
        "open_blockers": issue.open_blockers,
        "unblocks": issue.unblocks,
      }),
    );
  }
  json!({ "issues": out })
}

/// Jev from the environment, or why it is unavailable.
///
/// # Errors
/// `TYPESAFE_API_KEY` is unset, or the client cannot be built.
pub(crate) fn jev_from_env(env: &toolu_runtime::env::Env) -> Result<Jev, String> {
  if env.get("TYPESAFE_API_KEY").is_none_or(str::is_empty) {
    return Err("TYPESAFE_API_KEY not set".to_owned());
  }
  Jev::from_env(env, toolu_jev_client::Config::default()).map_err(|err| err.to_string())
}

/// Jev's tier per issue key.
///
/// # Errors
/// The call fails; the message says why, for the route note.
pub(crate) fn jev_scores(
  jev: &Jev,
  issues: &[Evidence],
) -> Result<BTreeMap<String, Score>, String> {
  let asked = Questions::parse(&questions(issues).to_string()).map_err(|err| err.to_string())?;
  let state = State::structured(&state(issues).to_string());
  let reply = jev
    .ask(&state, toolu_jev_client::DEFAULT_MODEL, &asked)
    .map_err(|err| format!("jev failed: {err}"))?;
  let mut scores = BTreeMap::new();
  for (key, answer) in reply.answers {
    if let Answer::Score {
      score, confidence, ..
    } = answer
    {
      scores.insert(
        key,
        Score {
          tier: tier_from_score(score),
          expected: Some(score),
          confidence: Some(confidence),
          source: "jev".to_owned(),
        },
      );
    }
  }
  Ok(scores)
}

#[cfg(test)]
#[path = "tests/score_test.rs"]
mod tests;
