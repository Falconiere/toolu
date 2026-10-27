//! Validate ready worker profiles and ask Jev to choose one.

use std::collections::HashSet;

use clap::ArgMatches;
use serde::Deserialize;
use toolu_jev_client::{Answer, Config, DEFAULT_MODEL, Jev, Question, Questions, Reply, State};
use toolu_runtime::env::Env;

use crate::{read_json, required};

const NO_ROUTE: &str = "no-suitable-route";

#[derive(Debug, Deserialize)]
struct Candidate {
  id: String,
  host: String,
  model: String,
  effort: String,
  description: String,
}

pub(crate) fn run(args: &ArgMatches, env: &Env) -> Result<String, String> {
  let candidates: Vec<Candidate> =
    serde_json::from_value(read_json(required(args, "candidates")?)?)
      .map_err(|e| format!("invalid candidates: {e}"))?;
  validate(&candidates)?;
  let evidence_path = required(args, "evidence")?;
  let evidence = std::fs::read_to_string(evidence_path)
    .map_err(|e| format!("cannot read {evidence_path}: {e}"))?;
  if evidence.trim().is_empty() {
    return Err("evidence must not be empty".to_owned());
  }
  let descriptions: Vec<String> = candidates
    .iter()
    .map(|c| {
      format!(
        "host: {}; model: {}; effort: {}; {}",
        c.host, c.model, c.effort, c.description
      )
    })
    .collect();
  let mut options: Vec<(&str, Option<&str>)> = candidates
    .iter()
    .zip(&descriptions)
    .map(|(c, desc)| (c.id.as_str(), Some(desc.as_str())))
    .collect();
  options.push((
    NO_ROUTE,
    Some("No supplied profile is suitable for the evidence"),
  ));
  let question = Question::choice("Choose a supplied ready worker with sufficient capability to audit and update documentation. Prefer lower cost or effort when supported by the supplied evidence; do not infer prices. Choose no suitable route if none fits.", &options).map_err(|e| e.to_string())?;
  let jev = Jev::from_env(env, Config::default()).map_err(|e| e.to_string())?;
  let reply = jev
    .ask(
      &State::structured(&evidence),
      DEFAULT_MODEL,
      &Questions::single("route", question),
    )
    .map_err(|e| e.to_string())?;
  select(&candidates, &reply).map(|value| value.to_string())
}

fn validate(candidates: &[Candidate]) -> Result<(), String> {
  if !(1..=254).contains(&candidates.len()) {
    return Err("candidates must contain 1 to 254 profiles".to_owned());
  }
  let mut ids = HashSet::new();
  for c in candidates {
    let safe = !c.id.is_empty()
      && c
        .id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
    let effort = match c.host.as_str() {
      "cursor" => c.effort == "model",
      "claude" | "codex" | "muse" => matches!(
        c.effort.as_str(),
        "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
      ),
      _ => false,
    };
    if !safe
      || c.id == NO_ROUTE
      || !ids.insert(&c.id)
      || !effort
      || c.model.trim().is_empty()
      || c.description.trim().is_empty()
    {
      return Err(format!("invalid or duplicate candidate: {}", c.id));
    }
  }
  Ok(())
}

fn select(candidates: &[Candidate], reply: &Reply) -> Result<serde_json::Value, String> {
  let [(id, Answer::Choice { choice, .. })] = reply.answers.as_slice() else {
    return Err("Jev returned an invalid route answer".to_owned());
  };
  if id != "route" {
    return Err("Jev returned an invalid route answer".to_owned());
  }
  let c = candidates
    .iter()
    .find(|c| c.id == *choice)
    .ok_or_else(|| "Jev found no suitable route or returned an unknown candidate".to_owned())?;
  Ok(
    serde_json::json!({"source":"jev", "candidate_id":c.id, "host":c.host, "model":c.model, "effort":c.effort}),
  )
}

#[cfg(test)]
#[path = "tests/route_test.rs"]
mod tests;
