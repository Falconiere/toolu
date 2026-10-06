//! Plugin dependency checks (`dependencies.ts`). Claude Code resolves
//! `plugin.json` dependencies itself; Codex has none, so a dependent plugin
//! asks `codex plugin list --json` at `SessionStart` and warns with the exact
//! install command. The live listing is read, not the snapshot: toolu's own
//! `SessionStart` writes the snapshot, so it is absent exactly when the core is.

use serde_json::{Map, Value};
use toolu_protocol::host::Host;

use super::context::{render_hook_output, session_context};
use crate::host::roots::Roots;
use crate::process::commands::codex_plugin_list;

/// The core plugin every other plugin needs.
pub const CORE_PLUGIN: &str = "toolu@toolu";

/// Which warning [`codex_dependency_notice`] words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeStyle {
  /// The single-core wording of `check-toolu.sh`.
  Core,
  /// Every missing plugin, as `check-deps.sh` listed them.
  Each,
}

/// `(has(key) | not) or (.[key] == true)`.
fn flag_on(entry: &Map<String, Value>, key: &str) -> bool {
  entry
    .get(key)
    .is_none_or(|value| value == &Value::Bool(true))
}

/// jq `any(.installed[]?; .pluginId == $id and <installed> and <enabled>)`
/// under `-e`: an entry jq cannot index raises, which counts as not installed
/// unless an earlier entry already matched.
fn is_installed(listing: &Value, id: &str) -> bool {
  let entries: Vec<&Value> = match listing.get("installed") {
    Some(Value::Array(items)) => items.iter().collect(),
    Some(Value::Object(map)) => map.values().collect(),
    Some(_) | None => Vec::new(),
  };
  for entry in entries {
    let entry = match entry {
      Value::Null => continue,
      Value::Object(entry) => entry,
      Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => return false,
    };
    let named = entry.get("pluginId").and_then(Value::as_str) == Some(id);
    if named && flag_on(entry, "installed") && flag_on(entry, "enabled") {
      return true;
    }
  }
  false
}

/// The `required` ids Codex does not list as installed and enabled, in order.
/// `None` means "do not check": not Codex, `PLUGIN_ROOT` unset, or the CLI
/// missing or failing. Output that is not JSON counts every id as missing.
pub fn codex_missing_plugins(required: &[&str], roots: &Roots) -> Option<Vec<String>> {
  if roots.host() != Host::Codex || roots.env().get("PLUGIN_ROOT").is_none() {
    return None;
  }
  let text = codex_plugin_list(roots.env())?;
  let listing = serde_json::from_str::<Value>(&text).ok();
  let missing = required.iter().filter(|id| {
    !listing
      .as_ref()
      .is_some_and(|listing| is_installed(listing, id))
  });
  Some(missing.map(|id| (*id).to_owned()).collect())
}

fn install_command(spec: &str) -> String {
  format!("codex plugin add {spec}")
}

/// The core-dependency warning of `check-toolu.sh`.
pub fn requires_core_warning() -> String {
  format!(
    "WARN: this plugin requires the toolu core. Install it first with: {}",
    install_command(CORE_PLUGIN)
  )
}

/// The multi-plugin warning of `check-deps.sh`: each missing id with its install command.
pub fn requires_plugins_warning(missing: &[String]) -> String {
  let parts: Vec<String> = missing
    .iter()
    .map(|id| format!(" {id} (install with: {})", install_command(id)))
    .collect();
  format!("WARN: this plugin requires{}", parts.concat())
}

/// The whole dependency hook: the pretty `SessionStart` context naming what is
/// missing, or `None` when there is nothing to say.
pub fn codex_dependency_notice(
  required: &[&str],
  style: NoticeStyle,
  roots: &Roots,
) -> Option<String> {
  let missing = codex_missing_plugins(required, roots).filter(|missing| !missing.is_empty())?;
  let text = match style {
    NoticeStyle::Core => requires_core_warning(),
    NoticeStyle::Each => requires_plugins_warning(&missing),
  };
  Some(render_hook_output(
    &session_context("SessionStart", &text)?,
    true,
  ))
}

#[cfg(test)]
#[path = "tests/dependencies_test.rs"]
mod tests;
