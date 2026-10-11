//! Jev's complexity score per fix item, asked in process, replayed from a
//! file, or skipped with the note the plan carries.

use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_jev_client::{Answer, Config, DEFAULT_MODEL, Error, Jev, Questions, State};
use toolu_runtime::env::Env;

use super::items::Item;
use crate::error::{Failure, Result};

const CRITERIA: [&str; 4] = [
  "Trivial: a typo, wording, rename, import, or one-line change with an obvious answer.",
  "Standard: a bounded edit in one function or file with a clear answer, possibly with its colocated test.",
  "Complex: spans several files, or needs new tests, concurrency handling, or behavior design.",
  "Critical: security or data-loss risk, cross-cutting architecture, or an ambiguous request that needs deep reasoning.",
];

const FALLBACK: &str = "heuristic tiers used";

/// Where the scores come from.
pub(crate) enum Scoring<'a> {
  /// No scores: Jev is disabled.
  Off,
  /// A recorded answers document.
  Replay(&'a Path),
  /// Jev, with the key `env` holds.
  Ask(&'a Env, Config),
}

/// Answers by item id (`{score, confidence}`), and the note when there are none.
pub(crate) struct Scores {
  /// The answer per scored item.
  pub(crate) answers: Map<String, Value>,
  /// Why the items fell back to heuristic tiers.
  pub(crate) note: Option<String>,
}

fn fallback(reason: &str) -> Scores {
  Scores {
    answers: Map::new(),
    note: Some(format!("{reason}; {FALLBACK}")),
  }
}

fn object(entries: impl Iterator<Item = (String, Value)>) -> String {
  let fields: Vec<String> = entries
    .map(|(key, value)| format!("{}:{value}", json!(key)))
    .collect();
  format!("{{{}}}", fields.join(","))
}

/// The state and questions `route-fix` sends, items in file order.
pub(crate) fn request(items: &[Item]) -> (String, String) {
  let state = object(items.iter().map(|item| {
    let fields =
      json!({"task": item.task, "path": item.path, "severity": item.severity, "kind": item.kind});
    (item.id.clone(), fields)
  }));
  let questions = object(items.iter().map(|item| {
    let instructions = format!(
      "How much implementation complexity does the review fix `items[\"{}\"]` carry for one \
       coding agent? Judge from its task, path, severity and kind only.",
      item.id
    );
    let question = json!({"type": "score", "instructions": instructions, "criteria": CRITERIA});
    (item.id.clone(), question)
  }));
  (format!("{{\"items\":{state}}}"), questions)
}

fn ask(
  items: &[Item],
  env: &Env,
  config: Config,
) -> std::result::Result<Map<String, Value>, Error> {
  let jev = Jev::from_env(env, config)?;
  let (state, questions) = request(items);
  let reply = jev.ask(
    &State::structured(&state),
    DEFAULT_MODEL,
    &Questions::parse(&questions)?,
  )?;
  let scores = reply
    .answers
    .into_iter()
    .filter_map(|(id, answer)| match answer {
      Answer::Score {
        score, confidence, ..
      } => Some((id, json!({"score": score, "confidence": confidence}))),
      Answer::Noul { .. } | Answer::Choice { .. } => None,
    });
  Ok(scores.collect())
}

/// The scores `scoring` yields for `items`.
///
/// # Errors
/// `usage` for a replay file that is not a JSON object.
pub(crate) fn scores(items: &[Item], scoring: Scoring<'_>) -> Result<Scores> {
  match scoring {
    Scoring::Off => Ok(fallback("jev disabled")),
    Scoring::Replay(path) => {
      let text = std::fs::read_to_string(path).unwrap_or_default();
      match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(answers)) => Ok(Scores {
          answers,
          note: None,
        }),
        Ok(_) | Err(_) => Err(Failure::usage(format!(
          "--jev-answers-in is not a JSON object: {}",
          path.display()
        ))),
      }
    }
    Scoring::Ask(env, config) => match ask(items, env, config) {
      Ok(answers) => Ok(Scores {
        answers,
        note: None,
      }),
      Err(Error::MissingKey) => Ok(fallback("jev unavailable (TYPESAFE_API_KEY not set)")),
      Err(error) => {
        let detail: String = error
          .to_string()
          .replace('\n', " ")
          .chars()
          .take(160)
          .collect();
        Ok(fallback(&format!("jev failed: {detail}")))
      }
    },
  }
}

#[cfg(test)]
#[path = "tests/jev_test.rs"]
mod tests;
