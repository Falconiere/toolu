//! The hook entries `cargo xtask bench-hooks` measures: one per
//! `<plugin>/<hook name>` of every plugin's `hooks.json`, the first
//! registration winning, sorted by id; then one per `muse:<plugin>/<hook id>`
//! of every `.muse-plugin/plugin.json`, sorted the same way.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use toolu_protocol::launcher::hook_name;
use toolu_protocol::launcher::muse::RESOLVER_DIR;

/// The id prefix of a Muse entry.
const MUSE: &str = "muse:";

/// How a host starts the hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Launch {
  /// The `hooks.json` command, run under `/bin/sh -c`.
  Command(String),
  /// The Muse resolver file `/bin/sh` runs, by its absolute path, as Muse passes it.
  Resolver(PathBuf),
}

/// One hook entry and the launcher command hosts run for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookEntry {
  /// `<plugin>/<hook name>`, the payload and budget key.
  pub(crate) id: String,
  /// The event it is first registered under.
  pub(crate) event: String,
  /// The plugin directory, exported to the hook as its host's plugin root.
  pub(crate) root: PathBuf,
  /// How the host starts it.
  pub(crate) launch: Launch,
}

impl HookEntry {
  /// The plugin directory name the id carries.
  pub(crate) fn plugin(&self) -> &str {
    let id = self.id.strip_prefix(MUSE).unwrap_or(&self.id);
    id.split_once('/').map_or("toolu", |(plugin, _)| plugin)
  }

  /// The spawn that is measured: `bin [<plugin>] hook <name> --event <event>
  /// --plugin-root <root>` under `/bin/sh -c 'exec "$0" "$@"'`, the shape the budgets of
  /// `benchmarks/hook-budgets.json` were measured in. The launcher's own resolution
  /// (its `--hook-protocol` probe) is left out until the budgets are measured with it.
  /// A Muse entry is `/bin/sh <resolver>`: the generated launcher, `--hook-protocol`
  /// probe included, as Muse runs it.
  pub(crate) fn argv(&self, bin: &Path) -> Vec<String> {
    if let Launch::Resolver(resolver) = &self.launch {
      return vec!["/bin/sh".to_owned(), resolver.display().to_string()];
    }
    let (plugin, name) = self
      .id
      .split_once('/')
      .unwrap_or(("toolu", self.id.as_str()));
    let mut argv = vec![
      "/bin/sh".to_owned(),
      "-c".to_owned(),
      "exec \"$0\" \"$@\"".to_owned(),
      bin.display().to_string(),
    ];
    if plugin != "toolu" {
      argv.push(plugin.to_owned());
    }
    argv.extend([
      "hook".to_owned(),
      name.to_owned(),
      "--event".to_owned(),
      self.event.clone(),
      "--plugin-root".to_owned(),
      self.root.display().to_string(),
    ]);
    argv
  }
}

/// Every entry under `<root>/plugins`: the `hooks.json` ones, then the Muse ones.
pub(crate) fn discover(root: &Path) -> Result<Vec<HookEntry>, String> {
  let mut found: Vec<HookEntry> = Vec::new();
  for dir in plugin_dirs(root, "hooks/hooks.json")? {
    for entry in entries_of(&dir)? {
      if !found.iter().any(|known| known.id == entry.id) {
        found.push(entry);
      }
    }
  }
  found.sort_by(|left, right| left.id.cmp(&right.id));
  let mut muse = Vec::new();
  for dir in plugin_dirs(root, ".muse-plugin/plugin.json")? {
    muse.extend(muse_entries_of(&dir)?);
  }
  muse.sort_by(|left, right| left.id.cmp(&right.id));
  found.extend(muse);
  Ok(found)
}

/// The plugin directories that hold `file`, sorted.
fn plugin_dirs(root: &Path, file: &str) -> Result<Vec<PathBuf>, String> {
  let plugins = root.join("plugins");
  let mut dirs: Vec<PathBuf> = std::fs::read_dir(&plugins)
    .map_err(|err| format!("cannot list {}: {err}", plugins.display()))?
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| path.join(file).is_file())
    .collect();
  dirs.sort();
  Ok(dirs)
}

fn plugin_name(dir: &Path) -> Result<&str, String> {
  dir
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| format!("{}: no plugin name", dir.display()))
}

/// The `capabilities.hooks` of `dir`'s Muse manifest; each must run its generated resolver.
fn muse_entries_of(dir: &Path) -> Result<Vec<HookEntry>, String> {
  let file = dir.join(".muse-plugin/plugin.json");
  let text = std::fs::read_to_string(&file).map_err(|err| format!("{}: {err}", file.display()))?;
  let doc: Value =
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", file.display()))?;
  let plugin = plugin_name(dir)?;
  let hooks = doc
    .pointer("/capabilities/hooks")
    .and_then(Value::as_array)
    .map(Vec::as_slice)
    .unwrap_or_default();
  let mut found = Vec::new();
  for hook in hooks {
    let text_of = |key: &str| hook.get(key).and_then(Value::as_str).unwrap_or_default();
    let (id, event) = (text_of("id"), text_of("event"));
    let resolver = format!("{RESOLVER_DIR}/{id}");
    let command: Vec<&str> = hook
      .get("command")
      .and_then(Value::as_array)
      .map(|words| words.iter().filter_map(Value::as_str).collect())
      .unwrap_or_default();
    if id.is_empty() || command != ["/bin/sh", resolver.as_str()] {
      return Err(format!(
        "{}: Muse hook {id} is not /bin/sh {resolver}",
        file.display()
      ));
    }
    found.push(HookEntry {
      id: format!("{MUSE}{plugin}/{id}"),
      event: event.to_owned(),
      root: dir.to_path_buf(),
      launch: Launch::Resolver(dir.join(&resolver)),
    });
  }
  Ok(found)
}

fn entries_of(dir: &Path) -> Result<Vec<HookEntry>, String> {
  let file = dir.join("hooks/hooks.json");
  let text = std::fs::read_to_string(&file).map_err(|err| format!("{}: {err}", file.display()))?;
  let doc: Value =
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", file.display()))?;
  let plugin = plugin_name(dir)?;
  let events = doc
    .get("hooks")
    .and_then(Value::as_object)
    .cloned()
    .unwrap_or_default();
  let mut found = Vec::new();
  for (event, groups) in &events {
    for command in commands(groups) {
      let name = hook_name(&command).ok_or_else(|| {
        format!(
          "{}: a {event} hook is not the native launcher",
          file.display()
        )
      })?;
      found.push(HookEntry {
        id: format!("{plugin}/{name}"),
        event: event.clone(),
        root: dir.to_path_buf(),
        launch: Launch::Command(command),
      });
    }
  }
  Ok(found)
}

fn commands(groups: &Value) -> Vec<String> {
  let groups = groups.as_array().map(Vec::as_slice).unwrap_or_default();
  groups
    .iter()
    .filter_map(|group| group.get("hooks").and_then(Value::as_array))
    .flatten()
    .filter_map(|hook| hook.get("command").and_then(Value::as_str))
    .map(str::to_owned)
    .collect()
}

/// The payloads must cover exactly the discovered entries; `only` must name known ones.
pub(crate) fn select(
  entries: Vec<HookEntry>,
  payloads: &Map<String, Value>,
  only: &[String],
) -> Result<Vec<HookEntry>, String> {
  let missing: Vec<&str> = entries
    .iter()
    .filter(|entry| !payloads.contains_key(&entry.id))
    .map(|entry| entry.id.as_str())
    .collect();
  let stale: Vec<&str> = payloads
    .keys()
    .filter(|id| !entries.iter().any(|entry| &entry.id == *id))
    .map(String::as_str)
    .collect();
  if !missing.is_empty() || !stale.is_empty() {
    return Err(format!(
      "payloads: no payload for [{}]; payloads for unknown entries [{}]",
      missing.join(", "),
      stale.join(", ")
    ));
  }
  if let Some(unknown) = only
    .iter()
    .find(|id| !entries.iter().any(|entry| &entry.id == *id))
  {
    return Err(format!("--only names an unknown entry: {unknown}"));
  }
  Ok(
    entries
      .into_iter()
      .filter(|entry| only.is_empty() || only.contains(&entry.id))
      .collect(),
  )
}

#[cfg(test)]
#[path = "tests/bench_hooks_entries_test.rs"]
mod tests;
