//! Persist remote dispatch independently of graphs and controller restarts.

use clap::{Arg, ArgMatches};
use serde_json::{Value, json};
use std::path::Path;
use toolu_mux::hc::remote::Target;
use toolu_runtime::env::Env;

/// The paired remote settings accepted at epic creation and registration.
pub(crate) fn args() -> [Arg; 2] {
  [
    Arg::new("machine")
      .long("machine")
      .value_name("NAME")
      .requires("remote-repo")
      .help("Existing Herdr machine for workers; requires herdr-app"),
    Arg::new("remote-repo")
      .long("remote-repo")
      .value_name("PATH")
      .requires("machine")
      .help("Existing absolute checkout on the worker machine"),
  ]
}

/// Save an explicitly supplied pair, retaining earlier settings when omitted.
pub(crate) fn configure(matches: &ArgMatches, dir: &Path, env: &Env) -> Result<(), String> {
  let machine = matches
    .try_get_one::<String>("machine")
    .ok()
    .flatten()
    .map(String::as_str);
  let repo = matches
    .try_get_one::<String>("remote-repo")
    .ok()
    .flatten()
    .map(String::as_str);
  let Some(target) = Target::new(machine, repo).map_err(|e| e.message)? else {
    return Ok(());
  };
  if toolu_mux::backend::configured(env)? != toolu_mux::Backend::HerdrApp {
    return Err("--machine and --remote-repo require multiplexer herdr-app".into());
  }
  let doc = json!({"backend":"herdr-app","machine":target.machine,"remote_repo":target.repo});
  let previous = load(dir)?;
  if previous.is_object() && previous != doc {
    return Err("remote target already recorded; preserve it for recovery".into());
  }
  crate::disk::write_value(&dir.join("target.json"), &doc)
}

/// Read the run target, refusing damaged state.
pub(crate) fn load(dir: &Path) -> Result<Value, String> {
  let path = dir.join("target.json");
  if matches!(path.try_exists(), Ok(false)) {
    return Ok(Value::Null);
  }
  let doc = crate::disk::read_value(&path)?;
  if !doc.is_object() || doc.get("backend").and_then(Value::as_str) != Some("herdr-app") {
    return Err("invalid epic target.json".into());
  }
  Target::new(
    doc.get("machine").and_then(Value::as_str),
    doc.get("remote_repo").and_then(Value::as_str),
  )
  .map_err(|e| e.message)?
  .ok_or("missing remote target")?;
  Ok(doc)
}

#[cfg(test)]
#[path = "tests/hc_target_test.rs"]
mod tests;
