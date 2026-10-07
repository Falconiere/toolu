//! The `Rule` trait: a compiled-in registry module (`RegistryModule` and
//! `RegistryContext` in `registry-types.ts`), run in process by the registry
//! runner (#418) when its manifest is installed and its matcher fits.

use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;

use super::RegistryEvent;
use crate::env::Env;

/// What an edit did to one path (`EDIT_OPERATIONS` in `state-schema.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOperation {
  /// A new file.
  Add,
  /// A changed file.
  Update,
  /// A removed file.
  Delete,
  /// A whole-file write.
  Write,
  /// A rename.
  Move,
}

/// One path of a multi-file patch the runner split into its own event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditSplit<'a> {
  /// What the patch did to the path.
  pub operation: EditOperation,
  /// The record's `from` (a move destination's source), else empty: `TOOLU_EDIT_FROM`.
  pub from: &'a str,
  /// The record's `moved_to` (a move source's destination), else empty: `TOOLU_EDIT_MOVED_TO`.
  pub moved_to: &'a str,
}

/// What a rule sees besides the event.
#[derive(Debug, Clone, Copy)]
pub struct RuleContext<'a> {
  /// The host.
  pub host: Host,
  /// The hook's environment.
  pub env: &'a Env,
  /// The host's config root.
  pub config_root: &'a Path,
  /// The project root.
  pub project_root: &'a Path,
  /// The hook process's working directory.
  pub cwd: Option<&'a Path>,
  /// The explicit plugin root, when a hook command supplied one.
  pub plugin_root: Option<&'a Path>,
  /// The host's raw hook payload, for ports that must match bash byte for byte.
  pub raw: &'a Map<String, Value>,
  /// Set when the runner split a multi-file patch into one event per path.
  pub edit: Option<EditSplit<'a>>,
}

/// A compiled-in registry module.
pub trait Rule: Send + Sync {
  /// The owning plugin, `name@marketplace`; its manifests' spec.
  fn spec(&self) -> &str;
  /// The rule name; its manifests' name.
  fn name(&self) -> &str;
  /// The event it runs on.
  fn event(&self) -> RegistryEvent;
  /// Whether the rule has anything to say about this event, such as a file it
  /// owns; the runner calls [`Rule::run`] only when it does. Every event by default.
  fn applies(&self, _event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> bool {
    true
  }
  /// Decides one event.
  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision;
}

#[cfg(test)]
#[path = "tests/rule_test.rs"]
mod tests;
