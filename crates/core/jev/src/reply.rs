//! Jev's reply, checked as `plugins/jev/hooks/src/jev/response.ts` checks it:
//! a typed answer for every question, or no judgment at all.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use toolu_runtime::json::ordered::Ordered;

use crate::{Error, Questions};

/// Tokens the call used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
  /// Input tokens.
  pub input_tokens: u64,
  /// Output tokens.
  pub output_tokens: u64,
}

/// One typed answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
  /// The probability of yes.
  Noul {
    /// In [0, 1].
    noul: f64,
  },
  /// The chosen option.
  Choice {
    /// One of the question's option keys.
    choice: String,
    /// A distribution over the option keys.
    probabilities: BTreeMap<String, f64>,
    /// How concentrated the distribution is, in [0, 1].
    confidence: f64,
  },
  /// A rating on the question's levels.
  Score {
    /// In [0, levels - 1]; fractional between levels.
    score: f64,
    /// The levels by index, "0" upward.
    legend: BTreeMap<String, Value>,
    /// A distribution over the level indices.
    probabilities: BTreeMap<String, f64>,
    /// How concentrated the distribution is, in [0, 1].
    confidence: f64,
  },
}

/// A checked reply.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
  /// The model that answered.
  pub model: String,
  /// The tokens it used.
  pub usage: Usage,
  /// The answers by question id, in question order.
  pub answers: Vec<(String, Answer)>,
  /// The reply text as received, for printing it unchanged.
  pub body: String,
}

/// Check `body` against `questions`.
pub(crate) fn parse(body: &str, questions: &Questions) -> Result<Reply, Error> {
  let envelope: Value = serde_json::from_str(body).map_err(|_err| Error::InvalidResponse)?;
  let model = envelope
    .get("model")
    .and_then(Value::as_str)
    .filter(|model| !model.is_empty())
    .ok_or(Error::InvalidResponse)?;
  let usage = envelope.get("usage").ok_or(Error::InvalidResponse)?;
  let usage = Usage {
    input_tokens: count(usage.get("input_tokens"))?,
    output_tokens: count(usage.get("output_tokens"))?,
  };
  let answers = object(envelope.get("answers"))?;
  let entries = questions.entries();
  if answers.len() != entries.len() || entries.iter().any(|(id, _)| !answers.contains_key(id)) {
    return Err(Error::InvalidResponse);
  }
  let answers = entries
    .iter()
    .map(|(id, question)| Ok((id.clone(), answer(question, answers.get(id))?)))
    .collect::<Result<_, Error>>()?;
  Ok(Reply {
    model: model.to_owned(),
    usage,
    answers,
    body: body.to_owned(),
  })
}

fn answer(question: &Ordered, answer: Option<&Value>) -> Result<Answer, Error> {
  let answer = object(answer)?;
  let Some(Ordered::String(kind)) = question.get("type") else {
    return Err(Error::InvalidResponse);
  };
  let kind = kind.as_str();
  if answer.get("type").and_then(Value::as_str) != Some(kind) {
    return Err(Error::InvalidResponse);
  }
  match kind {
    "noul" => Ok(Answer::Noul {
      noul: probability(answer.get("noul"))?,
    }),
    "choice" => choice(question, answer),
    "score" => score(question, answer),
    _ => Err(Error::InvalidResponse),
  }
}

fn choice(question: &Ordered, answer: &Map<String, Value>) -> Result<Answer, Error> {
  let Some(Ordered::Object(options)) = question.get("criteria") else {
    return Err(Error::InvalidResponse);
  };
  let keys: Vec<String> = options.iter().map(|(key, _)| key.clone()).collect();
  let choice = answer
    .get("choice")
    .and_then(Value::as_str)
    .filter(|choice| keys.iter().any(|key| key == choice))
    .ok_or(Error::InvalidResponse)?;
  Ok(Answer::Choice {
    choice: choice.to_owned(),
    probabilities: distribution(answer.get("probabilities"), &keys)?,
    confidence: probability(answer.get("confidence"))?,
  })
}

fn score(question: &Ordered, answer: &Map<String, Value>) -> Result<Answer, Error> {
  let Some(Ordered::Array(levels)) = question.get("criteria") else {
    return Err(Error::InvalidResponse);
  };
  let keys: Vec<String> = (0..levels.len()).map(|index| index.to_string()).collect();
  let top = f64::from(
    u32::try_from(levels.len().saturating_sub(1)).map_err(|_err| Error::InvalidResponse)?,
  );
  let score = answer
    .get("score")
    .and_then(Value::as_f64)
    .filter(|score| (0.0..=top).contains(score))
    .ok_or(Error::InvalidResponse)?;
  let legend = object(answer.get("legend"))?;
  if legend.len() != keys.len() || keys.iter().any(|key| !legend.contains_key(key)) {
    return Err(Error::InvalidResponse);
  }
  Ok(Answer::Score {
    score,
    legend: legend
      .iter()
      .map(|(key, value)| (key.clone(), value.clone()))
      .collect(),
    probabilities: distribution(answer.get("probabilities"), &keys)?,
    confidence: probability(answer.get("confidence"))?,
  })
}

fn object(value: Option<&Value>) -> Result<&Map<String, Value>, Error> {
  value
    .and_then(Value::as_object)
    .ok_or(Error::InvalidResponse)
}

fn probability(value: Option<&Value>) -> Result<f64, Error> {
  value
    .and_then(Value::as_f64)
    .filter(|number| (0.0..=1.0).contains(number))
    .ok_or(Error::InvalidResponse)
}

/// Probabilities over exactly `keys`, summing to 1 within 1e-6.
fn distribution(value: Option<&Value>, keys: &[String]) -> Result<BTreeMap<String, f64>, Error> {
  let map = object(value)?;
  if map.len() != keys.len() || keys.iter().any(|key| !map.contains_key(key)) {
    return Err(Error::InvalidResponse);
  }
  let probabilities = map
    .iter()
    .map(|(key, value)| Ok((key.clone(), probability(Some(value))?)))
    .collect::<Result<BTreeMap<_, _>, Error>>()?;
  let sum: f64 = probabilities.values().sum();
  if (sum - 1.0).abs() < 0.000_001 {
    Ok(probabilities)
  } else {
    Err(Error::InvalidResponse)
  }
}

/// A count `Number.isInteger` accepts and that is not negative; one above
/// `u64::MAX` saturates.
fn count(value: Option<&Value>) -> Result<u64, Error> {
  let value = value.ok_or(Error::InvalidResponse)?;
  if let Some(count) = value.as_u64() {
    return Ok(count);
  }
  let number = value
    .as_f64()
    .filter(|number| number.is_finite() && *number >= 0.0 && number.fract() == 0.0)
    .ok_or(Error::InvalidResponse)?;
  if number < 1.0 {
    // 0 and -0, which `Number.isInteger` accepts and `{:.0}` prints as "-0".
    return Ok(0);
  }
  Ok(format!("{number:.0}").parse().unwrap_or(u64::MAX))
}

#[cfg(test)]
#[path = "tests/reply_test.rs"]
mod tests;
