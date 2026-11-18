//! Generate a Qwen Code extension from the release's shared plugin assets.

use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::launcher::MARKER;

use super::store::{read_hooks, version_of};

/// Write Qwen's manifest into the staged copy. The hook root names the final
/// install directory and reaches the shell as data, even when it has spaces.
pub(super) fn write(source: &Path, staged: &Path, target: &Path, name: &str) -> Result<(), String> {
  let target = std::path::absolute(target)
    .map_err(|err| format!("{name}: cannot resolve extension directory: {err}"))?;
  let root = target
    .to_str()
    .ok_or_else(|| format!("{name}: extension directory is not UTF-8"))?;
  let version = version_of(source).ok_or_else(|| format!("{name}: no plugin version"))?;
  let mut hooks = read_hooks(source, name)?;
  let hooks = if hooks.is_null() {
    Map::new()
  } else {
    let events = hooks.get_mut("hooks").and_then(Value::as_object_mut);
    let events = events.ok_or_else(|| format!("{name}: expected an object of hook events"))?;
    adapt_hooks(events, root).map_err(|detail| format!("{name}: {detail}"))?;
    std::mem::take(events)
  };
  expand_commands(&staged.join("commands"), root)?;
  let path = staged.join("qwen-extension.json");
  let manifest = json!({ "name": name, "version": version, "hooks": hooks });
  std::fs::write(&path, manifest.to_string())
    .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

fn adapt_hooks(events: &mut Map<String, Value>, root: &str) -> Result<(), String> {
  for (event, groups) in events {
    let groups = groups
      .as_array_mut()
      .ok_or_else(|| format!("{event}: expected hook groups"))?;
    for group in groups {
      let hooks = group
        .get_mut("hooks")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("{event}: expected a list of hooks"))?;
      for hook in hooks {
        let hook = hook
          .as_object_mut()
          .ok_or_else(|| format!("{event}: expected a hook object"))?;
        let command = hook
          .get("command")
          .and_then(Value::as_str)
          .filter(|command| command.contains(MARKER) && command.contains("${CLAUDE_PLUGIN_ROOT}"))
          .ok_or_else(|| format!("{event}: expected a native toolu hook"))?;
        let command = command
          .replacen(MARKER, "--host qwen --hook-protocol", 1)
          .replace("${CLAUDE_PLUGIN_ROOT}", "${TOOLU_PLUGIN_ROOT}");
        hook.insert("command".to_owned(), command.into());
        hook.insert(
          "env".to_owned(),
          json!({ "TOOLU_HOST_OVERRIDE": "qwen", "TOOLU_PLUGIN_ROOT": root }),
        );
      }
    }
  }
  Ok(())
}

/// Qwen reads local Markdown commands as written; resolve their file paths.
fn expand_commands(dir: &Path, root: &str) -> Result<(), String> {
  if !dir.is_dir() {
    return Ok(());
  }
  let entries =
    std::fs::read_dir(dir).map_err(|err| format!("cannot read {}: {err}", dir.display()))?;
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    if path.is_dir() {
      expand_commands(&path, root)?;
    } else if path.extension().is_some_and(|ext| ext == "md") {
      let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
      if text.contains("${CLAUDE_PLUGIN_ROOT}/") {
        let text = text.replace("${CLAUDE_PLUGIN_ROOT}/", &format!("{root}/"));
        std::fs::write(&path, text)
          .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
      }
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/qwen_test.rs"]
mod tests;
