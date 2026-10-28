//! The guarded-slice diff the judge scenarios review, and Jev replies for it.

use serde_json::{Value, json};

use super::rig::Rig;

pub(crate) const GUARD: &str = "if items.is_empty() { return None; }";

pub(crate) fn rig() -> Result<Rig, String> {
  let base = format!("fn last(items: &[u8]) -> Option<u8> {{\n{GUARD}\nSome(items[0])\n}}\n");
  let change =
    format!("fn last(items: &[u8]) -> Option<u8> {{\n{GUARD}\nSome(items[items.len() - 1])\n}}\n");
  Rig::new(&[("src/lib.rs", base)], &[("src/lib.rs", change)])
}

/// A Jev reply choosing, per key, `choice` with probability `p` (the rest spread).
pub(crate) fn choices(answers: &[(&str, &str, f64)]) -> String {
  let answers: serde_json::Map<String, Value> = answers
    .iter()
    .map(|(key, choice, p)| {
      let others = ["supported", "contradicted", "insufficient"]
        .into_iter()
        .filter(|option| option != choice);
      let mut probabilities = serde_json::Map::new();
      probabilities.insert((*choice).to_owned(), json!(p));
      for other in others {
        probabilities.insert(other.to_owned(), json!((1.0 - p) / 2.0));
      }
      let answer = json!({"type": "choice", "choice": choice, "probabilities": probabilities, "confidence": p});
      ((*key).to_owned(), answer)
    })
    .collect();
  json!({"model": "typesafe/jev-1.13", "usage": {"input_tokens": 30, "output_tokens": 3, "cost": 0.002},
         "answers": answers})
  .to_string()
}
