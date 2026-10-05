//! The layer table (`crates/xtask/layers.json`) and the role each crate plays in it.

use std::fmt;
use std::path::{Component, Path};

use serde::Deserialize;

/// `crates/xtask/layers.json`: core layers lowest first, rule crates, and the
/// three crates with a fixed role.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerTable {
  /// Core crate directory names under `crates/core/`, grouped by layer, lowest first.
  pub core: Vec<Vec<String>>,
  /// Rule crate directory names under `crates/`; only `hub` may depend on them.
  pub rules: Vec<String>,
  /// The plugin crate that links the rule crates (`crates/toolu`).
  pub hub: String,
  /// The only crate that builds a binary (`crates/cli`).
  pub binary: String,
  /// The workspace tooling crate (`crates/xtask`).
  pub tooling: String,
}

/// What a crate is allowed to depend on, decided by its directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
  /// `crates/core/<name>` at the given layer index.
  Core(usize),
  /// `crates/<rule>`: a rule crate.
  Rule,
  /// `crates/<hub>`: the plugin crate that links rule crates.
  Hub,
  /// Any other `crates/<name>`.
  Plugin,
  /// `crates/<binary>`.
  Cli,
  /// `crates/<tooling>`.
  Tooling,
}

impl fmt::Display for Role {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Role::Core(layer) => write!(f, "core layer {layer}"),
      Role::Rule => f.write_str("rule crate"),
      Role::Hub => f.write_str("hub plugin crate"),
      Role::Plugin => f.write_str("plugin crate"),
      Role::Cli => f.write_str("cli crate"),
      Role::Tooling => f.write_str("tooling crate"),
    }
  }
}

impl LayerTable {
  /// Parse the table from JSON text.
  pub fn parse(text: &str) -> Result<Self, String> {
    serde_json::from_str(text).map_err(|err| format!("invalid layer table: {err}"))
  }

  /// The role of the crate in `dir`, relative to the workspace root.
  pub fn role(&self, dir: &Path) -> Result<Role, String> {
    let parts: Vec<&str> = dir
      .components()
      .map(|part| match part {
        Component::Normal(name) => name.to_str().unwrap_or(""),
        _ => "",
      })
      .collect();
    match parts.as_slice() {
      ["crates", "core", name] => self.core_layer(name).map(Role::Core).ok_or_else(|| {
        format!(
          "{} is a core crate with no layer in layers.json",
          dir.display()
        )
      }),
      ["crates", name] if *name != "core" => Ok(self.named_role(name)),
      _ => Err(format!(
        "{} is not a crates/<name> or crates/core/<name> directory",
        dir.display()
      )),
    }
  }

  fn core_layer(&self, name: &str) -> Option<usize> {
    self
      .core
      .iter()
      .position(|layer| layer.iter().any(|entry| entry == name))
  }

  fn named_role(&self, name: &str) -> Role {
    if name == self.binary {
      Role::Cli
    } else if name == self.tooling {
      Role::Tooling
    } else if name == self.hub {
      Role::Hub
    } else if self.rules.iter().any(|rule| rule == name) {
      Role::Rule
    } else {
      Role::Plugin
    }
  }
}

/// Why `from` may not depend on `to`, or `None` when the edge is allowed.
pub fn forbidden_edge(from: Role, to: Role) -> Option<&'static str> {
  match (from, to) {
    (_, Role::Cli) => Some("nothing may depend on the cli crate"),
    (_, Role::Tooling) => Some("nothing may depend on the tooling crate"),
    (Role::Core(from), Role::Core(to)) if to < from => None,
    (Role::Core(_), _) => Some("a core crate may depend only on lower core layers"),
    (Role::Hub, Role::Rule) => None,
    (_, Role::Rule) => Some("only the hub crate (crates/toolu) may depend on a rule crate"),
    (Role::Cli | Role::Tooling, _) | (_, Role::Core(_)) => None,
    (_, Role::Hub | Role::Plugin) => Some("a plugin crate may not depend on another plugin crate"),
  }
}

/// Whether a crate in `role` may build a binary target.
pub fn may_build_binary(role: Role) -> bool {
  matches!(role, Role::Cli | Role::Tooling)
}
