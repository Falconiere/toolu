//! The `prBabysit` block of the user's and the project's `toolu.config.json`,
//! merged project over user and validated.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use toolu_runtime::host::roots::Roots;

use super::host::{Choice, Fixer, safe};
use crate::error::{Failure, Result};

/// The four fix tiers, cheapest first.
pub(crate) const TIERS: [&str; 4] = ["trivial", "standard", "complex", "critical"];

/// The validated fixer settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settings {
  /// `dispatch: "inline"`: the controller fixes every item itself.
  pub(crate) inline: bool,
  /// The hosts fixers may run on, in preference order.
  pub(crate) hosts: Vec<Fixer>,
  /// Per tier index, the hosts to try first.
  pub(crate) prefer: BTreeMap<usize, Vec<Fixer>>,
  /// Per host, the model and effort of each tier.
  pub(crate) routing: BTreeMap<Fixer, Vec<Choice>>,
  /// Fixers run without approval prompts.
  pub(crate) unattended: bool,
  /// Jev scores the items' tiers.
  pub(crate) jev: bool,
}

fn invalid(message: impl Into<String>) -> Failure {
  Failure::new("config_invalid", message)
}

/// The settings for a controller running on `controller`, from the config
/// files `roots` names for `cwd`'s project.
///
/// # Errors
/// `config_invalid` for a `prBabysit` block that fails validation.
pub(crate) fn load(roots: &Roots, cwd: Option<&Path>, controller: Fixer) -> Result<Settings> {
  let user = block(&roots.config_root().join("toolu.config.json"))?;
  let project = match roots.project_config_path(cwd) {
    Some(path) => block(&path)?,
    None => Map::new(),
  };
  parse(&merge(user, project), controller)
}

fn block(path: &PathBuf) -> Result<Map<String, Value>> {
  let parsed = std::fs::read_to_string(path)
    .ok()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
  let Some(Value::Object(mut document)) = parsed else {
    return Ok(Map::new());
  };
  match document.remove("prBabysit") {
    None => Ok(Map::new()),
    Some(Value::Object(block)) => Ok(block),
    Some(_) => {
      let file = path.display().to_string();
      Err(invalid(format!("prBabysit must be an object (in {file})")).with("file", file))
    }
  }
}

fn merge(mut base: Map<String, Value>, over: Map<String, Value>) -> Map<String, Value> {
  for (key, value) in over {
    let merged = match (base.remove(&key), value) {
      (Some(Value::Object(old)), Value::Object(new)) => Value::Object(merge(old, new)),
      (_, value) => value,
    };
    base.insert(key, merged);
  }
  base
}

fn host_list(value: &Value, place: &str) -> Result<Vec<Fixer>> {
  let Value::Array(names) = value else {
    return Err(invalid(format!(
      "prBabysit: {place} must be an array of hosts"
    )));
  };
  let mut hosts = Vec::new();
  for name in names {
    let host = Fixer::parse(name.as_str().unwrap_or_default())?;
    if !hosts.contains(&host) {
      hosts.push(host);
    }
  }
  Ok(hosts)
}

fn prefer(raw: Option<&Value>) -> Result<BTreeMap<usize, Vec<Fixer>>> {
  let mut prefer = BTreeMap::new();
  let Some(raw) = raw else {
    return Ok(prefer);
  };
  let Value::Object(tiers) = raw else {
    return Err(invalid("prBabysit: prefer must be an object"));
  };
  for (tier, list) in tiers {
    let Some(index) = TIERS.iter().position(|name| name == tier) else {
      return Err(invalid(format!(
        "prBabysit: prefer key {tier} is not a tier"
      )));
    };
    prefer.insert(index, host_list(list, &format!("prefer.{tier}"))?);
  }
  Ok(prefer)
}

fn choice(value: &Value) -> Option<Choice> {
  let Value::Object(fields) = value else {
    return None;
  };
  let field = |key: &str| match fields.get(key) {
    None | Some(Value::Null) => Some(None),
    Some(Value::String(text)) if safe(text) => Some(Some(text.clone())),
    Some(_) => None,
  };
  Some(Choice {
    model: field("model")?,
    effort: field("effort")?,
  })
}

fn routing(raw: Option<&Value>) -> Result<BTreeMap<Fixer, Vec<Choice>>> {
  let mut routing: BTreeMap<Fixer, Vec<Choice>> = Fixer::ALL
    .iter()
    .map(|host| (*host, host.defaults()))
    .collect();
  let Some(raw) = raw else {
    return Ok(routing);
  };
  let Value::Object(rows) = raw else {
    return Err(invalid("prBabysit: routing must be an object"));
  };
  for (name, row) in rows {
    let host = Fixer::parse(name)?;
    let cells = row.as_array().filter(|cells| cells.len() == TIERS.len());
    let Some(cells) = cells else {
      return Err(invalid(format!(
        "prBabysit: routing.{name} must list 4 tiers"
      )));
    };
    let choices: Option<Vec<Choice>> = cells.iter().map(choice).collect();
    let Some(choices) = choices else {
      return Err(invalid(format!(
        "prBabysit: routing.{name} has a non-string or shell-unsafe model/effort"
      )));
    };
    routing.insert(host, choices);
  }
  Ok(routing)
}

fn flag(raw: &Map<String, Value>, key: &str) -> Result<bool> {
  match raw.get(key) {
    None => Ok(true),
    Some(Value::Bool(value)) => Ok(*value),
    Some(_) => Err(invalid(format!("prBabysit: {key} must be true or false"))),
  }
}

/// Validates a merged `prBabysit` block.
///
/// # Errors
/// `config_invalid` naming the first field that fails.
pub(crate) fn parse(raw: &Map<String, Value>, controller: Fixer) -> Result<Settings> {
  let inline = match raw.get("dispatch").and_then(Value::as_str) {
    None if raw.get("dispatch").is_none() => false,
    Some("herdr") => false,
    Some("inline") => true,
    _ => return Err(invalid("prBabysit: dispatch must be herdr or inline")),
  };
  let hosts = match raw.get("hosts") {
    None => vec![controller],
    Some(list) => host_list(list, "hosts")?,
  };
  if hosts.is_empty() {
    return Err(invalid("prBabysit: hosts must name at least one host"));
  }
  Ok(Settings {
    inline,
    hosts,
    prefer: prefer(raw.get("prefer"))?,
    routing: routing(raw.get("routing"))?,
    unattended: flag(raw, "unattended")?,
    jev: flag(raw, "jev")?,
  })
}

#[cfg(test)]
#[path = "tests/config_test.rs"]
mod tests;
