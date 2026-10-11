//! `route-fix`: score each review fix item, give it a tier, and group the
//! items by tier onto a fixer host, model and effort — or inline, with why.

pub(crate) mod items;
pub(crate) mod jev;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::error::{Failure, Result};
use crate::fixer::config::{Settings, TIERS};
use crate::fixer::host::{Choice, Fixer};
use items::Item;
use jev::{Scores, Scoring};

const CLASSES: [&str; 4] = [
  "mechanical",
  "implementation",
  "architecture",
  "architecture",
];

/// One `route-fix` run.
pub(crate) struct Request<'a> {
  /// The items document.
  pub(crate) items: &'a Path,
  /// The babysit state, for its host cooldowns.
  pub(crate) state_file: Option<&'a Path>,
  /// Items to route one tier up.
  pub(crate) raise: &'a [String],
  /// The clock, in Unix seconds.
  pub(crate) now: u64,
}

struct Scored<'a> {
  item: &'a Item,
  tier: usize,
  answer: Option<(Value, Value)>,
  raised: bool,
}

/// Whether `name` is an executable file on `env`'s `PATH`.
pub(crate) fn on_path(env: &Env, name: &str) -> bool {
  let path = env.get("PATH").unwrap_or_default();
  path.split(':').filter(|dir| !dir.is_empty()).any(|dir| {
    let metadata = std::fs::metadata(Path::new(dir).join(name));
    metadata.is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
  })
}

fn cooling(state_file: Option<&Path>, now: u64) -> Vec<String> {
  let state = state_file
    .and_then(|path| std::fs::read_to_string(path).ok())
    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
  let Some(Value::Object(cooldowns)) =
    state.and_then(|mut state| state.get_mut("hostCooldowns").map(Value::take))
  else {
    return Vec::new();
  };
  let until = |entry: &Value| {
    let until = entry.get("until").and_then(Value::as_str)?;
    toolu_state::time::parse_iso(until)?
      .duration_since(std::time::UNIX_EPOCH)
      .ok()
  };
  let cooling = cooldowns
    .iter()
    .filter(|(_, entry)| until(entry).is_some_and(|at| at.as_secs() > now));
  cooling.map(|(host, _)| host.clone()).collect()
}

fn scored<'a>(items: &'a [Item], scores: &Scores, raise: &[String]) -> Vec<Scored<'a>> {
  let answer_of = |item: &Item| {
    let answer = scores.answers.get(&item.id)?;
    let score = answer.get("score").filter(|score| score.is_number())?;
    Some((
      score.clone(),
      answer.get("confidence").cloned().unwrap_or(Value::Null),
    ))
  };
  let tier = |answer: &Option<(Value, Value)>, item: &Item| match answer {
    Some((score, _)) => {
      let rounded = (score.as_f64().unwrap_or_default() + 0.15).round();
      [1.0, 2.0, 3.0]
        .iter()
        .filter(|step| rounded >= **step)
        .count()
    }
    None => items::heuristic(item),
  };
  let ranked = items.iter().map(|item| {
    let answer = answer_of(item);
    let raised = raise.contains(&item.id);
    let base = tier(&answer, item);
    Scored {
      item,
      tier: if raised { (base + 1).min(3) } else { base },
      answer,
      raised,
    }
  });
  ranked.collect()
}

fn item_json(scored: &Scored<'_>) -> Value {
  let (score, confidence) = scored.answer.clone().unwrap_or((Value::Null, Value::Null));
  let source = if scored.answer.is_some() {
    "jev"
  } else {
    "heuristic"
  };
  json!({
    "id": scored.item.id,
    "tier": TIERS.get(scored.tier),
    "score": score,
    "confidence": confidence,
    "raised": scored.raised,
    "source": source,
  })
}

fn source(scored: &[Scored<'_>]) -> &'static str {
  if scored.iter().all(|scored| scored.answer.is_some()) {
    "jev"
  } else if scored.iter().all(|scored| scored.answer.is_none()) {
    "heuristic"
  } else {
    "mixed"
  }
}

fn group(
  settings: &Settings,
  eligible: &[Fixer],
  tier: usize,
  inline: bool,
) -> (Option<Fixer>, Choice) {
  if inline {
    return (None, Choice::default());
  }
  let preferred = settings
    .prefer
    .get(&tier)
    .map(Vec::as_slice)
    .unwrap_or_default();
  let host = preferred
    .iter()
    .find(|host| eligible.contains(host))
    .or_else(|| eligible.first());
  let choice = host.and_then(|host| settings.routing.get(host)?.get(tier).cloned());
  (host.copied(), choice.unwrap_or_default())
}

fn groups(
  settings: &Settings,
  eligible: &[Fixer],
  scored: &[Scored<'_>],
  inline: bool,
) -> Vec<Value> {
  let tiers = (0..TIERS.len())
    .rev()
    .filter(|tier| scored.iter().any(|item| item.tier == *tier));
  let groups = tiers.enumerate().map(|(index, tier)| {
    let (host, choice) = group(settings, eligible, tier, inline);
    let members: Vec<&str> = scored
      .iter()
      .filter(|item| item.tier == tier)
      .map(|item| item.item.id.as_str())
      .collect();
    json!({
      "seq": index + 1,
      "tier": TIERS.get(tier),
      "class": CLASSES.get(tier),
      "host": host.map(Fixer::name),
      "model": choice.model,
      "effort": choice.effort,
      "items": members,
    })
  });
  groups.collect()
}

fn inline_reason(
  settings: &Settings,
  eligible: &[Fixer],
  cooling: &[String],
  missing: &[&str],
) -> Option<String> {
  if settings.inline {
    return Some("prBabysit.dispatch is inline".into());
  }
  if !eligible.is_empty() {
    return None;
  }
  let mut why = vec!["no fixer host available".to_owned()];
  if !cooling.is_empty() {
    why.push(format!("(cooling: {})", cooling.join(", ")));
  }
  if !missing.is_empty() {
    why.push(format!("(CLI not on PATH: {})", missing.join(", ")));
  }
  Some(why.join(" "))
}

/// The route plan for `request` under `settings`.
///
/// # Errors
/// `plan_invalid` for a missing or invalid items file, `usage` for a bad replay.
pub(crate) fn route(
  request: &Request<'_>,
  settings: &Settings,
  env: &Env,
  scoring: Scoring<'_>,
) -> Result<Value> {
  let text = std::fs::read_to_string(request.items).unwrap_or_default();
  let document = serde_json::from_str::<Value>(&text).map_err(|error| {
    let file = request.items.display();
    Failure::new(
      "plan_invalid",
      format!("items file is missing or not JSON: {file}"),
    )
    .with("cause", error.to_string())
  })?;
  let items = items::parse(&document, request.raise)?;
  let missing: Vec<Fixer> = settings
    .hosts
    .iter()
    .copied()
    .filter(|host| !on_path(env, host.cli()))
    .collect();
  let cooling = cooling(request.state_file, request.now);
  let eligible: Vec<Fixer> = settings
    .hosts
    .iter()
    .copied()
    .filter(|host| !missing.contains(host) && !cooling.iter().any(|name| name == host.name()))
    .collect();
  let jev_scores = jev::scores(&items, scoring)?;
  let scored = scored(&items, &jev_scores, request.raise);
  let missing: Vec<&str> = missing.iter().map(|host| host.name()).collect();
  let inline = inline_reason(settings, &eligible, &cooling, &missing);
  let mut notes: Vec<String> = jev_scores.note.into_iter().chain(inline.clone()).collect();
  if !eligible.is_empty() && !missing.is_empty() {
    notes.push(format!("dropped (CLI not on PATH): {}", missing.join(", ")));
  }
  if !eligible.is_empty() && !cooling.is_empty() {
    notes.push(format!("cooling: {}", cooling.join(", ")));
  }
  let note = (!notes.is_empty()).then(|| notes.join("; "));
  let items: Vec<Value> = scored.iter().map(item_json).collect();
  Ok(json!({
    "version": 1,
    "dispatch": if inline.is_some() { "inline" } else { "herdr" },
    "unattended": settings.unattended,
    "source": source(&scored),
    "note": note,
    "items": items,
    "groups": groups(settings, &eligible, &scored, inline.is_some()),
  }))
}

#[cfg(test)]
#[path = "tests/route_test.rs"]
mod tests;
