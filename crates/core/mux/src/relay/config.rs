//! Persistent relay and worker registration.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use super::durable;
use serde::{Deserialize, Serialize};
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, with_lock_strict};

/// Registered worker machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
  /// Stable machine name.
  pub name: String,
  /// Absolute workspace root on the worker.
  pub root: String,
  /// Supported agent names.
  pub agents: Vec<String>,
  /// Maximum concurrent assignments.
  pub capacity: u32,
  /// Whether new work is disabled.
  pub draining: bool,
}

/// Versioned relay configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
  /// Configuration format version.
  pub version: u32,
  /// Relay endpoint, if configured.
  pub relay: Option<String>,
  /// Unique authority generation, retained after retirement.
  #[serde(default)]
  pub generation: Option<String>,
  /// Workers indexed by name.
  pub machines: BTreeMap<String, Machine>,
  /// Per-origin setup commands, run in the prepared checkout.
  #[serde(default)]
  pub recipes: BTreeMap<String, Vec<Vec<String>>>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      version: 1,
      relay: None,
      generation: None,
      machines: BTreeMap::new(),
      recipes: BTreeMap::new(),
    }
  }
}

/// Configuration directory for delegation state.
pub fn root(env: &Env) -> PathBuf {
  let base = env.get("TOOLU_CONFIG_DIR").map_or_else(
    || {
      env
        .get("XDG_CONFIG_HOME")
        .map_or_else(|| env.home().join(".config"), PathBuf::from)
        .join("toolu")
    },
    PathBuf::from,
  );
  base.join("delegation")
}

/// Load and validate the registry, or return its default when absent.
///
/// # Errors
/// Returns an error on unreadable, malformed, or invalid configuration.
pub fn load(root: &Path) -> Result<Config, String> {
  let path = root.join("config.json");
  let body = match std::fs::read(&path) {
    Ok(body) => body,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
    Err(error) => return Err(format!("reading {}: {error}", path.display())),
  };
  let config = serde_json::from_slice(&body)
    .map_err(|error| format!("invalid {}: {error}", path.display()))?;
  validate(&config)?;
  Ok(config)
}

/// Apply a serialized, durable configuration change.
///
/// # Errors
/// Returns an error if the lock, change, validation, or write fails.
pub fn update(
  root: &Path,
  change: impl FnOnce(&mut Config) -> Result<(), String>,
) -> Result<Config, String> {
  durable::ensure_dir(root)?;
  let path = root.join("config.json");
  let options = LockOptions {
    stale: Duration::MAX,
    ..LockOptions::default()
  };
  with_lock_strict(&path, options, || {
    let mut config = load(root)?;
    let before = config.clone();
    change(&mut config)?;
    validate(&config)?;
    if config != before {
      durable::write(
        &path,
        &serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
      )?;
    }
    Ok(config)
  })?
}

/// Register or replace a valid machine.
///
/// # Errors
/// Rejects invalid machine details.
pub fn register(config: &mut Config, machine: Machine) -> Result<(), String> {
  validate_machine(&machine)?;
  config.machines.insert(machine.name.clone(), machine);
  Ok(())
}

/// Validate the format and every registered machine.
///
/// # Errors
/// Rejects unsupported versions and invalid registrations.
pub fn validate(config: &Config) -> Result<(), String> {
  if config.version != 1 {
    return Err(format!("unsupported config version {}", config.version));
  }
  for (name, machine) in &config.machines {
    if name != &machine.name {
      return Err(format!("machine key {name} does not match its name"));
    }
    validate_machine(machine)?;
  }
  if config.recipes.iter().any(|(origin, commands)| {
    origin.is_empty()
      || commands
        .iter()
        .any(|argv| argv.first().is_none_or(String::is_empty))
  }) {
    return Err("invalid setup recipe".into());
  }
  Ok(())
}

fn validate_machine(machine: &Machine) -> Result<(), String> {
  if machine.name.is_empty()
    || machine.name.len() > 128
    || !machine
      .name
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
  {
    return Err("invalid machine name".into());
  }
  let path = Path::new(&machine.root);
  if !path.is_absolute()
    || path
      .components()
      .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    || path.to_string_lossy() != machine.root
    || machine.root.contains("//")
    || machine
      .root
      .split('/')
      .any(|part| part == "." || part == "..")
    || (machine.root.len() > 1 && machine.root.ends_with('/'))
  {
    return Err("workspace root must be an absolute normalized path".into());
  }
  if machine.agents.is_empty()
    || machine.agents.iter().any(|agent| {
      !matches!(
        agent.as_str(),
        "claude" | "codex" | "muse" | "cursor" | "opencode"
      )
    })
  {
    return Err("machine must have supported agents".into());
  }
  if machine.capacity == 0 {
    return Err("machine capacity must be positive".into());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/config_test.rs"]
mod tests;
