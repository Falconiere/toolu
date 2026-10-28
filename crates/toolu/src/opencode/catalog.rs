//! A plugin's skills and commands as the shim registers them with `OpenCode`'s
//! `ctx.skill.transform` and `ctx.command.transform`, namespaced
//! `<plugin>:<name>` as Claude Code names them:
//!
//! - a skill is `skills/<dir>/SKILL.md`: `{id, name, description, path,
//!   content}`, its frontmatter's `name` and `description` and the body after it;
//! - a command is `commands/<name>.md`: `{name, description, template}`, its
//!   frontmatter's `description` or else its first heading, and the body with
//!   `${CLAUDE_PLUGIN_ROOT}` replaced by the plugin directory. The shim
//!   substitutes `$ARGUMENTS`.
//!
//! The plugin name is its manifest's `name`, else the directory's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The catalog document of the plugin at `root`.
///
/// # Errors
/// When a skill or command file cannot be read.
pub(crate) fn catalog(root: &Path) -> Result<Value, String> {
  let plugin = plugin_name(root);
  let mut skills = Vec::new();
  for dir in entries(&root.join("skills"))? {
    let path = dir.join("SKILL.md");
    if path.is_file() {
      skills.push(skill(&plugin, &path)?);
    }
  }
  let mut commands = Vec::new();
  for path in entries(&root.join("commands"))? {
    if path.extension().is_some_and(|ext| ext == "md") && path.is_file() {
      commands.push(command(&plugin, &path, root)?);
    }
  }
  Ok(json!({"plugin": plugin, "skills": skills, "commands": commands}))
}

fn plugin_name(root: &Path) -> String {
  let manifest = std::fs::read_to_string(root.join(".claude-plugin/plugin.json"))
    .ok()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
  manifest
    .as_ref()
    .and_then(|doc| doc.get("name"))
    .and_then(Value::as_str)
    .map(str::to_owned)
    .or_else(|| {
      root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
    })
    .unwrap_or_default()
}

/// The entries of `dir`, sorted; none when it does not exist. An entry that cannot
/// be read fails the catalog rather than leaving a skill or command out.
fn entries(dir: &Path) -> Result<Vec<PathBuf>, String> {
  let read = match std::fs::read_dir(dir) {
    Ok(read) => read,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(format!("cannot read {}: {err}", dir.display())),
  };
  let mut paths = Vec::new();
  for entry in read {
    let entry = entry.map_err(|err| format!("cannot read {}: {err}", dir.display()))?;
    paths.push(entry.path());
  }
  paths.sort();
  Ok(paths)
}

fn read(path: &Path) -> Result<String, String> {
  std::fs::read_to_string(path).map_err(|err| format!("cannot read {}: {err}", path.display()))
}

fn skill(plugin: &str, path: &Path) -> Result<Value, String> {
  let text = read(path)?;
  let (front, body) = frontmatter(&text);
  let fallback = path
    .parent()
    .and_then(Path::file_name)
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default();
  let name = front.get("name").cloned().unwrap_or(fallback);
  let id = format!("{plugin}:{name}");
  Ok(json!({
    "id": id,
    "name": id,
    "description": front.get("description").cloned().unwrap_or_default(),
    "path": path.display().to_string(),
    "content": body.trim_start(),
  }))
}

fn command(plugin: &str, path: &Path, root: &Path) -> Result<Value, String> {
  let text = read(path)?;
  let (front, body) = frontmatter(&text);
  let stem = path
    .file_stem()
    .map(|stem| stem.to_string_lossy().into_owned())
    .unwrap_or_default();
  let description = front
    .get("description")
    .cloned()
    .or_else(|| heading(body))
    .unwrap_or_else(|| stem.clone());
  let template = body
    .trim_start()
    .replace("${CLAUDE_PLUGIN_ROOT}", &root.display().to_string());
  Ok(json!({
    "name": format!("{plugin}:{stem}"),
    "description": description,
    "template": template,
  }))
}

/// The `key: value` lines of a leading `---` block, and the text after it.
fn frontmatter(text: &str) -> (BTreeMap<String, String>, &str) {
  let mut fields = BTreeMap::new();
  let Some(rest) = text.strip_prefix("---\n") else {
    return (fields, text);
  };
  let Some((block, body)) = rest
    .split_once("\n---\n")
    .or_else(|| rest.strip_suffix("\n---").map(|block| (block, "")))
  else {
    return (fields, text);
  };
  for line in block.lines() {
    if let Some((key, value)) = line.split_once(':') {
      let value = value.trim();
      let unquoted = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value);
      fields.insert(key.trim().to_owned(), unquoted.to_owned());
    }
  }
  (fields, body)
}

/// The text of the first `# ` heading.
fn heading(body: &str) -> Option<String> {
  body
    .lines()
    .find_map(|line| line.strip_prefix("# "))
    .map(|text| text.trim().to_owned())
    .filter(|text| !text.is_empty())
}

#[cfg(test)]
#[path = "tests/catalog_test.rs"]
mod tests;
