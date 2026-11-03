//! Trust toolu-owned attempt directories through each host's documented
//! trust store on the worker, so a launch never waits at a folder dialog.

use super::{ArgMatches, Env, Value, json, value};
use std::path::{Path, PathBuf};

/// Execute `delegate trust`.
pub(super) fn run(args: &ArgMatches, env: &Env) -> Result<Value, String> {
  let home = env
    .get("HOME")
    .filter(|h| !h.is_empty())
    .ok_or("HOME is unset")?;
  let mut paths = vec![value(args, "path")];
  if let Some(repo) = args.get_one::<String>("repo").filter(|r| !r.is_empty()) {
    paths.push(repo);
  }
  if paths.iter().any(|path| !Path::new(path).is_absolute()) {
    return Err("trusted paths must be absolute".into());
  }
  let store = match value(args, "agent") {
    "claude" => claude(env, Path::new(home), &paths)?,
    "codex" => vec![codex(env, Path::new(home), &paths)?],
    other => return Err(format!("no trust store for agent {other}")),
  };
  Ok(json!({"agent": value(args, "agent"), "trusted": paths, "store": store}))
}

/// Claude keeps `.claude.json` and `settings.json` under `$CLAUDE_CONFIG_DIR`,
/// else `~/.claude.json` and `~/.claude/settings.json`. The project entry
/// skips the folder dialog; the settings key skips the one-time bypass
/// permissions dialog that interactive `--dangerously-skip-permissions`
/// shows. Every other key stays.
fn claude(env: &Env, home: &Path, paths: &[&str]) -> Result<Vec<PathBuf>, String> {
  let (store, settings) = match env.get("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
    Some(dir) => (
      Path::new(dir).join(".claude.json"),
      Path::new(dir).join("settings.json"),
    ),
    None => (
      home.join(".claude.json"),
      home.join(".claude").join("settings.json"),
    ),
  };
  edit_json(&store, |root| {
    let projects = root
      .entry("projects")
      .or_insert_with(|| json!({}))
      .as_object_mut()
      .ok_or("projects is not an object")?;
    for path in paths {
      projects
        .entry(*path)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| format!("project {path} is not an object"))?
        .insert("hasTrustDialogAccepted".into(), Value::Bool(true));
    }
    Ok(())
  })?;
  edit_json(&settings, |root| {
    root.insert(
      "skipDangerousModePermissionPrompt".into(),
      Value::Bool(true),
    );
    Ok(())
  })?;
  Ok(vec![store, settings])
}

/// Change the top-level object of a JSON file, creating a missing or blank
/// file.
fn edit_json(
  file: &Path,
  edit: impl FnOnce(&mut serde_json::Map<String, Value>) -> Result<(), String>,
) -> Result<(), String> {
  let mut doc = match std::fs::read(file) {
    // A blank settings file is one an operator touched, not a broken one.
    Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => json!({}),
    Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", file.display()))?,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
    Err(error) => return Err(format!("{}: {error}", file.display())),
  };
  let root = doc
    .as_object_mut()
    .ok_or_else(|| format!("{} is not a JSON object", file.display()))?;
  edit(root).map_err(|e| format!("{}: {e}", file.display()))?;
  if let Some(parent) = file.parent() {
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
  }
  write(
    file,
    &serde_json::to_string(&doc).map_err(|e| e.to_string())?,
  )
}

/// `$CODEX_HOME/config.toml`: append `[projects."<path>"] trust_level =
/// "trusted"` for each path without a table, leaving the rest of the file as
/// the operator wrote it.
fn codex(env: &Env, home: &Path, paths: &[&str]) -> Result<PathBuf, String> {
  let dir = env
    .get("CODEX_HOME")
    .filter(|d| !d.is_empty())
    .map_or_else(|| home.join(".codex"), PathBuf::from);
  let file = dir.join("config.toml");
  let mut text = match std::fs::read_to_string(&file) {
    Ok(text) => text,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
    Err(error) => return Err(format!("{}: {error}", file.display())),
  };
  let before = text.len();
  for path in paths {
    let key = path.replace('\\', "\\\\").replace('"', "\\\"");
    let header = format!("[projects.\"{key}\"]");
    if text.lines().any(|line| line.trim() == header) {
      continue;
    }
    if !text.is_empty() && !text.ends_with('\n') {
      text.push('\n');
    }
    text.push('\n');
    text.push_str(&header);
    text.push_str("\ntrust_level = \"trusted\"\n");
  }
  if text.len() != before {
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    write(&file, &text)?;
  }
  Ok(file)
}

fn write(file: &Path, text: &str) -> Result<(), String> {
  if toolu_state::io::write_atomic(file, text) {
    Ok(())
  } else {
    Err(format!("cannot write {}", file.display()))
  }
}

#[cfg(test)]
#[path = "tests/trust_test.rs"]
mod tests;
