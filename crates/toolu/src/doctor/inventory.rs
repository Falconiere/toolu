//! Installed toolu plugins, one source per host.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value, json};
use toolu_engine::registry::gate::installed_plugins_path;
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::manifest;
use toolu_runtime::process::{Spec, run};

use super::checks::{Check, Status};

/// How long `codex plugin list --json` may run. Only a Codex host starts it.
const CODEX_LIST_TIMEOUT: Duration = Duration::from_secs(10);

/// One installed toolu plugin.
pub(super) struct Plugin {
  pub(super) name: String,
  pub(super) version: Option<String>,
  pub(super) root: Option<PathBuf>,
  pub(super) muse: Option<MuseHealth>,
}

/// Muse activation and approval state, separate from installation.
pub(super) struct MuseHealth {
  pub(super) active: bool,
  pub(super) enabled: bool,
  pub(super) approval: &'static str,
  pub(super) pending_hooks: Vec<String>,
}

/// Plugins the host could list, plus a warning when the inventory is incomplete.
pub(super) struct Inventory {
  pub(super) plugins: Vec<Plugin>,
  pub(super) warning: Option<String>,
}

impl Inventory {
  fn warn(warning: impl Into<String>) -> Inventory {
    Inventory {
      plugins: Vec::new(),
      warning: Some(warning.into()),
    }
  }

  /// The installed plugin names paired with their readable roots.
  pub(super) fn roots(&self) -> impl Iterator<Item = (&str, &Path)> {
    self.plugins.iter().filter_map(|plugin| {
      plugin
        .root
        .as_deref()
        .map(|root| (plugin.name.as_str(), root))
    })
  }
}

/// Read the host's toolu plugins.
pub(super) fn collect(roots: &Roots, cwd: &Path) -> Inventory {
  match roots.host() {
    Host::Claude => claude(roots, cwd),
    Host::Codex => codex(roots),
    Host::Muse => super::muse::collect(roots),
    Host::Cursor => super::cursor::collect(roots),
    Host::Opencode => super::opencode::collect(roots),
    Host::Hermes => Inventory::warn("plugin inventory is unknown for this host"),
  }
}

/// The plugins check. An unavailable or empty inventory warns and does not fail.
pub(super) fn plugins_check(inventory: &Inventory) -> Check {
  let plugins: Vec<Value> = inventory.plugins.iter().map(plugin_json).collect();
  let details = json!({ "plugins": plugins });
  let mut notices = inventory.warning.iter().cloned().collect::<Vec<_>>();
  notices.extend(inventory.plugins.iter().filter_map(muse_notice));
  if !notices.is_empty() {
    return Check::new("plugins", Status::Warn, notices.join("; "), None, details);
  }
  if inventory.plugins.is_empty() {
    return Check::new(
      "plugins",
      Status::Warn,
      "no toolu plugin is installed",
      None,
      details,
    );
  }
  let count = inventory.plugins.len();
  Check::new(
    "plugins",
    Status::Ok,
    format!("{count} toolu plugins"),
    None,
    details,
  )
}

fn muse_notice(plugin: &Plugin) -> Option<String> {
  let state = plugin.muse.as_ref()?;
  if !state.enabled {
    return Some(format!(
      "{} is disabled; muse plugins enable {}",
      plugin.name, plugin.name
    ));
  }
  if !state.active {
    return Some(format!(
      "{} is inactive; muse plugins inspect {}",
      plugin.name, plugin.name
    ));
  }
  if !state.pending_hooks.is_empty() {
    return Some(format!(
      "{} hooks are installed but inert; muse plugins approve {}",
      plugin.name, plugin.name
    ));
  }
  match state.approval {
    "needs-approval" => Some(format!(
      "{} capabilities need approval; muse plugins approve {}",
      plugin.name, plugin.name
    )),
    "unknown" => Some(format!("{} approval status is unavailable", plugin.name)),
    _ => None,
  }
}

fn plugin_json(plugin: &Plugin) -> Value {
  let mut map = Map::new();
  map.insert("name".to_owned(), json!(plugin.name));
  if let Some(version) = &plugin.version {
    map.insert("version".to_owned(), json!(version));
  }
  if let Some(root) = &plugin.root {
    map.insert("root".to_owned(), json!(root.display().to_string()));
  }
  if let Some(muse) = &plugin.muse {
    map.insert("active".to_owned(), json!(muse.active));
    map.insert("enabled".to_owned(), json!(muse.enabled));
    map.insert("approval".to_owned(), json!(muse.approval));
    map.insert("pendingHooks".to_owned(), json!(muse.pending_hooks));
  }
  Value::Object(map)
}

fn plugin_at(name: &str, root: PathBuf) -> Plugin {
  let version = manifest::read(&root).ok().map(|read| read.version);
  Plugin {
    name: name.to_owned(),
    version,
    root: Some(root),
    muse: None,
  }
}

fn claude(roots: &Roots, cwd: &Path) -> Inventory {
  let Ok(text) = std::fs::read_to_string(installed_plugins_path(roots.env())) else {
    return Inventory::warn("plugin inventory is unavailable");
  };
  let Ok(value) = serde_json::from_str::<Value>(&text) else {
    return Inventory::warn("plugin inventory is unavailable");
  };
  let Some(plugins) = value.get("plugins").and_then(Value::as_object) else {
    return Inventory::warn("plugin inventory is unavailable");
  };
  let project = roots.project_root(Some(cwd));
  let mut found = Vec::new();
  let mut warnings = Vec::new();
  for (spec, entries) in plugins {
    push_claude(spec, entries, project.as_deref(), &mut found, &mut warnings);
  }
  Inventory {
    plugins: found,
    warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
  }
}

fn push_claude(
  spec: &str,
  entries: &Value,
  project: Option<&Path>,
  found: &mut Vec<Plugin>,
  warnings: &mut Vec<String>,
) {
  let Some(name) = spec.strip_suffix("@toolu").filter(|name| !name.is_empty()) else {
    return;
  };
  let Some(entries) = entries.as_array() else {
    warnings.push(format!("{spec} is not an install list"));
    return;
  };
  for entry in entries {
    match claude_entry(name, entry, project) {
      ClaudeEntry::Skip => {}
      ClaudeEntry::MissingPath => warnings.push(format!("{spec} has no installPath")),
      ClaudeEntry::Plugin(plugin) => found.push(plugin),
    }
  }
}

enum ClaudeEntry {
  Skip,
  MissingPath,
  Plugin(Plugin),
}

fn claude_entry(name: &str, entry: &Value, project: Option<&Path>) -> ClaudeEntry {
  let Some(entry) = entry.as_object() else {
    return ClaudeEntry::Skip;
  };
  if !scope_kept(entry, project) {
    return ClaudeEntry::Skip;
  }
  match entry.get("installPath").and_then(Value::as_str) {
    Some(path) if !path.is_empty() => ClaudeEntry::Plugin(plugin_at(name, PathBuf::from(path))),
    Some(_) | None => ClaudeEntry::MissingPath,
  }
}

fn scope_kept(entry: &Map<String, Value>, project: Option<&Path>) -> bool {
  match entry.get("scope").and_then(Value::as_str) {
    Some("user") => true,
    Some("project" | "local") => entry
      .get("projectPath")
      .and_then(Value::as_str)
      .is_some_and(|path| project.is_some_and(|project| same_path(Path::new(path), project))),
    Some(_) | None => false,
  }
}

fn same_path(left: &Path, right: &Path) -> bool {
  let canon = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
  canon(left) == canon(right)
}

fn codex(roots: &Roots) -> Inventory {
  let mut spec = Spec::new(["codex", "plugin", "list", "--json"]);
  spec.timeout = CODEX_LIST_TIMEOUT;
  spec.env = Some(roots.env().clone());
  spec.max_output_bytes = 1_048_576;
  let Ok(output) = run(&spec) else {
    return Inventory::warn("plugin inventory is unavailable");
  };
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Inventory::warn("plugin inventory is unavailable");
  }
  let Some(list) = parse_codex(&output.stdout) else {
    return Inventory::warn("plugin inventory is unavailable");
  };
  let notices: Vec<String> = [
    (list.missing_path > 0).then(|| "a toolu plugin has no source.path".to_owned()),
    super::codex_pin::notice(roots.env(), env!("CARGO_PKG_VERSION")),
  ]
  .into_iter()
  .flatten()
  .collect();
  Inventory {
    plugins: list.plugins,
    warning: (!notices.is_empty()).then(|| notices.join("; ")),
  }
}

struct CodexList {
  plugins: Vec<Plugin>,
  missing_path: usize,
}

fn parse_codex(stdout: &str) -> Option<CodexList> {
  let value: Value = serde_json::from_str(stdout).ok()?;
  let installed = value.get("installed")?.as_array()?;
  let mut plugins = Vec::new();
  let mut missing_path = 0;
  for item in installed {
    let entry = item.as_object()?;
    if entry.get("marketplaceName").and_then(Value::as_str) != Some("toolu") {
      continue;
    }
    if !flag_on(entry, "installed") || !flag_on(entry, "enabled") {
      continue;
    }
    let name = entry.get("name").and_then(Value::as_str)?;
    match entry
      .get("source")
      .and_then(Value::as_object)
      .and_then(|source| source.get("path"))
      .and_then(Value::as_str)
    {
      Some(path) if !path.is_empty() => plugins.push(plugin_at(name, PathBuf::from(path))),
      Some(_) | None => missing_path += 1,
    }
  }
  Some(CodexList {
    plugins,
    missing_path,
  })
}

fn flag_on(entry: &Map<String, Value>, key: &str) -> bool {
  entry
    .get(key)
    .is_none_or(|value| value == &Value::Bool(true))
}

#[cfg(test)]
#[path = "tests/inventory_test.rs"]
mod tests;
